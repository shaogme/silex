use super::TagDef;
use heck::AsSnakeCase;

// --- Generation Logic ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagNamespace {
    Html,
    Svg,
}

pub fn generate_module_content(
    tags: &[TagDef],
    namespace: TagNamespace,
    forbidden_macros: &[String],
) -> String {
    let mut code = String::new();
    let module_name = match namespace {
        TagNamespace::Html => "html",
        TagNamespace::Svg => "svg",
    };
    let namespace_name = match namespace {
        TagNamespace::Html => "html",
        TagNamespace::Svg => "svg",
    };

    // --- Tags ---
    code.push_str("// --- Tags ---\n");
    for tag in tags {
        let fn_name = tag
            .func_name
            .clone()
            .unwrap_or_else(|| AsSnakeCase(&tag.struct_name).to_string());

        let kind = if tag.is_void { "void" } else { "non_void" };
        let trait_list = tag.traits.join(", ");

        // Generate define_tag! macro call
        code.push_str(&format!(
            "#[rustfmt::skip] silex_view::define_tag!({}, \"{}\", {}, {}, {}, [{}]);\n",
            tag.struct_name, tag.tag_name, namespace_name, fn_name, kind, trait_list
        ));
    }

    // --- Public Macros ---
    code.push_str("\n// --- Macros ---\n");
    for tag in tags {
        let fn_name = tag
            .func_name
            .clone()
            .unwrap_or_else(|| AsSnakeCase(&tag.struct_name).to_string());

        if !tag.is_void {
            let macro_name = if forbidden_macros.contains(&fn_name) {
                format!("svg_{}", fn_name)
            } else {
                fn_name.clone()
            };

            code.push_str(&format!(
                "#[rustfmt::skip] #[macro_export] macro_rules! {} {{\n",
                macro_name
            ));
            code.push_str(&format!(
                "    () => {{ $crate::{}::{}($crate::ViewNil) }};\n",
                module_name, fn_name
            ));
            code.push_str(&format!(
                "    ($($child:expr),+ $(,)?) => {{ $crate::{}::{}($crate::chain!($($child),+)) }};\n",
                module_name, fn_name
            ));
            code.push_str("}\n");
        }
    }

    code
}

#[cfg(test)]
mod tests {
    use super::super::TagDef;
    use super::{TagNamespace, generate_module_content};

    fn anchor_tag(struct_name: &str) -> TagDef {
        TagDef {
            struct_name: struct_name.to_string(),
            tag_name: "a".to_string(),
            func_name: None,
            is_void: false,
            traits: vec![],
        }
    }

    #[test]
    fn generated_tags_only_contain_view_metadata() {
        let html = generate_module_content(&[anchor_tag("A")], TagNamespace::Html, &[]);
        let svg = generate_module_content(&[anchor_tag("SvgA")], TagNamespace::Svg, &[]);

        assert!(html.contains("silex_view::define_tag!(A, \"a\", html, a, non_void"));
        assert!(svg.contains("silex_view::define_tag!(SvgA, \"a\", svg, svg_a, non_void"));
        assert!(!html.contains("web_sys"));
        assert!(!svg.contains("web_sys"));
    }

    #[test]
    fn generated_macros_use_chain_without_compat_alias() {
        let html = generate_module_content(&[anchor_tag("A")], TagNamespace::Html, &[]);
        let svg = generate_module_content(&[anchor_tag("SvgA")], TagNamespace::Svg, &[]);

        assert!(html.contains("$crate::chain!"));
        assert!(svg.contains("$crate::chain!"));
        assert!(!html.contains("view_chain"));
        assert!(!svg.contains("view_chain"));
    }
}
