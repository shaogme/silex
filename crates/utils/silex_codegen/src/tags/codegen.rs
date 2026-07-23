use super::TagDef;
use heck::AsSnakeCase;

// --- Generation Logic ---

pub fn generate_module_content(
    tags: &[TagDef],
    is_svg: bool,
    forbidden_macros: &[String],
) -> String {
    let mut code = String::new();
    let namespace = if is_svg { "svg" } else { "html" };
    let method_name = if is_svg { "new_svg" } else { "new" };

    // --- Tags ---
    code.push_str("// --- Tags ---\n");
    for tag in tags {
        let fn_name = tag
            .func_name
            .clone()
            .unwrap_or_else(|| AsSnakeCase(&tag.struct_name).to_string());

        let kind = if tag.is_void { "void" } else { "non_void" };
        let trait_list = tag.traits.join(", ");

        let dom_type = match tag.tag_name.as_str() {
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
            _ if is_svg => "web_sys::SvgElement",
            _ => "web_sys::HtmlElement",
        };

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
                namespace, fn_name
            ));
            code.push_str(&format!(
                "    ($($child:expr),+ $(,)?) => {{ $crate::{}::{}($crate::view_chain!($($child),+)) }};\n",
                namespace, fn_name
            ));
            code.push_str("}\n");
        }
    }

    code
}
