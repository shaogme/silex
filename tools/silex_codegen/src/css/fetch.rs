use super::types::{CssConfig, MdnCssProperty, MdnCssSyntax, ProcessedProp, PropGroup};
use heck::{AsPascalCase, AsSnakeCase};
use std::collections::HashMap;

pub fn fetch_css(whitelist: &[String]) -> Result<CssConfig, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("silex-codegen")
        .build()?;

    let props_url = "https://raw.githubusercontent.com/mdn/data/main/css/properties.json";
    let syntaxes_url = "https://raw.githubusercontent.com/mdn/data/main/css/syntaxes.json";

    println!("Downloading CSS properties from {}", props_url);
    let props_resp = client.get(props_url).send()?;
    if !props_resp.status().is_success() {
        return Err(format!(
            "Failed to fetch MDN CSS properties: {}",
            props_resp.status()
        )
        .into());
    }
    let raw_props: HashMap<String, MdnCssProperty> = props_resp.json()?;

    println!("Downloading CSS syntaxes from {}", syntaxes_url);
    let syntaxes_resp = client.get(syntaxes_url).send()?;
    if !syntaxes_resp.status().is_success() {
        return Err(format!(
            "Failed to fetch MDN CSS syntaxes: {}",
            syntaxes_resp.status()
        )
        .into());
    }
    let syntaxes: HashMap<String, MdnCssSyntax> = syntaxes_resp.json()?;
    let resolver = SyntaxResolver::new(&syntaxes);

    let mut properties = Vec::new();

    for (name, prop) in &raw_props {
        // Only standard properties, unless whitelisted
        if prop.status != "standard" && !whitelist.contains(name) {
            continue;
        }

        let method_name = AsSnakeCase(&name).to_string();
        let struct_name = AsPascalCase(&name).to_string();

        // Skip if empty or invalid identifiers (e.g. "--*")
        if method_name.is_empty() || struct_name.is_empty() || !is_valid_identifier(&method_name) {
            continue;
        }

        // Purely derive from MDN syntax
        let (group, keywords) = classify_property(name, prop, &resolver);

        properties.push(ProcessedProp {
            name: name.clone(),
            method_name,
            struct_name,
            group,
            keywords,
        });
    }

    // Sort for deterministic output
    properties.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(CssConfig {
        properties,
        syntaxes,
    })
}

fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    for c in chars {
        if !c.is_alphanumeric() && c != '_' {
            return false;
        }
    }
    true
}

fn classify_property(
    name: &str,
    prop: &MdnCssProperty,
    resolver: &SyntaxResolver,
) -> (PropGroup, Vec<String>) {
    let syntax = &prop.syntax;

    // 1. Determine Keywords (Recursively)
    let keywords = resolver.resolve_keywords(syntax);

    // 2. Automate Group Determination based on Syntax Patterns
    // Patterns for Shorthand/Multi-value properties:
    // - "||" (Double bar): multiple options in any order
    // - "&&" (Double ampersand): all options in any order
    // - "[ ... ]+" or "[ ... ]#" or "{1,4}": repeating components
    // - Space-separated components (e.g. "<length> <color>")

    let is_complex = syntax.contains("||")
        || syntax.contains("&&")
        || syntax.contains('{')
        || syntax.contains('+')
        || syntax.contains('#')
        || syntax.contains(' ') && !syntax.trim().is_empty();

    let group = if is_complex {
        // Special rule: if it looks like a shorthand but we only want to expose it as Dimension/Color
        // because it's essentially just a value with optional flags, we might refine this.
        // But for most common ones (margin, border, flex), Shorthand is safer.
        PropGroup::Shorthand
    } else if syntax.contains("<length")
        || syntax.contains("<percentage")
        || name.contains("width") && !name.contains("stroke")
        || name.contains("height")
        || syntax.contains("radius>")
        || name.contains("radius")
        || name == "zoom"
    {
        PropGroup::Dimension
    } else if syntax.contains("<color") || syntax.contains("color>") {
        PropGroup::Color
    } else if syntax.contains("<number") || syntax.contains("<integer") || name == "font-weight" {
        PropGroup::Number
    } else if !keywords.is_empty() && keywords.len() < 50 {
        PropGroup::Keyword
    } else {
        PropGroup::Custom
    };

    (group, keywords)
}

pub struct SyntaxResolver<'a> {
    pub syntaxes: &'a HashMap<String, MdnCssSyntax>,
}

impl<'a> SyntaxResolver<'a> {
    pub fn new(syntaxes: &'a HashMap<String, MdnCssSyntax>) -> Self {
        Self { syntaxes }
    }

    pub fn resolve_keywords(&self, syntax: &str) -> Vec<String> {
        let mut keywords = Vec::new();
        let mut visited = std::collections::HashSet::new();
        self.collect_keywords_recursive(syntax, &mut keywords, &mut visited);
        keywords.sort();
        keywords.dedup();
        keywords
    }

    fn collect_keywords_recursive(
        &self,
        syntax: &str,
        keywords: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
    ) {
        // Simple regex-like extraction of tokens
        let tokens = syntax.replace(['[', ']', '{', '}', '*', '+', '?', '#', ',', '(', ')'], " ");
        for part in tokens.split('|') {
            for subpart in part.split_whitespace() {
                let trimmed = subpart.trim().trim_matches('\'').trim_matches('\"');
                if trimmed.is_empty() {
                    continue;
                }

                // If it's a reference to another syntax <foo>
                if trimmed.starts_with('<') && trimmed.ends_with('>') {
                    let ref_name = &trimmed[1..trimmed.len() - 1];
                    if visited.insert(ref_name.to_string())
                        && let Some(ref_syntax) = self.syntaxes.get(ref_name)
                    {
                        self.collect_keywords_recursive(&ref_syntax.syntax, keywords, visited);
                    }
                } else if self.is_literal_keyword(trimmed) {
                    keywords.push(trimmed.to_string());
                }
            }
        }
    }

    fn is_literal_keyword(&self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let mut chars = s.chars();
        let first = chars.next().unwrap();

        // Must start with alphabet, and contain only alphanumeric or hyphen
        // Also exclude common CSS units/values types
        if !first.is_alphabetic() {
            return false;
        }

        if s == "inherit" || s == "initial" || s == "unset" || s == "revert" || s == "none" {
            return true;
        }

        if s.chars().all(|c| c.is_alphanumeric() || c == '-')
            && !s.contains('<')
            && !s.contains('>')
        {
            // Filter out some garbage or too abstract things
            let blacklisted = ["u", "v", "x", "y", "number", "integer", "string", "ident"];
            !blacklisted.contains(&s)
        } else {
            false
        }
    }
}
