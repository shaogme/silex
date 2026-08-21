use super::TagDef;
use heck::AsSnakeCase;

// --- Generation Logic ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagNamespace {
    Html,
    Svg,
}

fn dom_type(namespace: TagNamespace, tag_name: &str) -> &'static str {
    match namespace {
        TagNamespace::Svg => match tag_name {
            "a" => "web_sys::SvgaElement",
            _ => "web_sys::SvgElement",
        },
        TagNamespace::Html => match tag_name {
            "input" => "web_sys::HtmlInputElement",
            "button" => "web_sys::HtmlButtonElement",
            "textarea" => "web_sys::HtmlTextAreaElement",
            "select" => "web_sys::HtmlSelectElement",
            "option" => "web_sys::HtmlOptionElement",
            "optgroup" => "web_sys::HtmlOptGroupElement",
            "form" => "web_sys::HtmlFormElement",
            "a" => "web_sys::HtmlAnchorElement",
            "img" => "web_sys::HtmlImageElement",
            "canvas" => "web_sys::HtmlCanvasElement",
            "audio" => "web_sys::HtmlAudioElement",
            "video" => "web_sys::HtmlVideoElement",
            "dialog" => "web_sys::HtmlDialogElement",
            "details" => "web_sys::HtmlDetailsElement",
            "iframe" => "web_sys::HtmlIFrameElement",
            _ => "web_sys::HtmlElement",
        },
    }
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
    let method_name = match namespace {
        TagNamespace::Html => "new",
        TagNamespace::Svg => "new_svg",
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

        let dom_type = dom_type(namespace, &tag.tag_name);

        // Generate define_tag! macro call
        code.push_str(&format!(
            "#[rustfmt::skip] silex_dom::define_tag!({}, {}, \"{}\", {}, {}, {}, [{}]);\n",
            tag.struct_name, dom_type, tag.tag_name, fn_name, method_name, kind, trait_list
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
    use super::{TagNamespace, dom_type, generate_module_content};

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
    fn svg_anchor_uses_svg_anchor_dom_type() {
        assert_eq!(dom_type(TagNamespace::Svg, "a"), "web_sys::SvgaElement");
    }

    #[test]
    fn html_anchor_keeps_html_anchor_dom_type() {
        assert_eq!(
            dom_type(TagNamespace::Html, "a"),
            "web_sys::HtmlAnchorElement"
        );
    }

    #[test]
    fn namespace_controls_fallback_dom_type() {
        assert_eq!(dom_type(TagNamespace::Svg, "path"), "web_sys::SvgElement");
        assert_eq!(
            dom_type(TagNamespace::Html, "section"),
            "web_sys::HtmlElement"
        );
    }

    #[test]
    fn generated_anchor_types_follow_their_namespace() {
        let html = generate_module_content(&[anchor_tag("A")], TagNamespace::Html, &[]);
        let svg = generate_module_content(&[anchor_tag("SvgA")], TagNamespace::Svg, &[]);

        assert!(html.contains("web_sys::HtmlAnchorElement"));
        assert!(svg.contains("web_sys::SvgaElement"));
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
