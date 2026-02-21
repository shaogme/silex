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

    // Check if we will generate any attribute impls to avoid unused imports
    let has_impls = tags.iter().any(|t| {
        t.traits.iter().any(|trait_name| {
            matches!(
                trait_name.as_str(),
                "FormTag"
                    | "LabelTag"
                    | "AnchorTag"
                    | "MediaTag"
                    | "OpenTag"
                    | "TableCellTag"
                    | "TableHeaderTag"
            )
        })
    });

    if has_impls {
        code.push_str("use silex_dom::TypedElement;\n");
        code.push_str("use silex_dom::attribute::*;\n");
        code.push_str("use crate::attributes::*;\n");
        code.push_str("use wasm_bindgen::JsCast;\n\n");
    }

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
            "silex_dom::define_tag!({}, \"{}\", {}, {}, {}, [{}]);\n",
            tag.struct_name, tag.tag_name, fn_name, method_name, kind, trait_list
        ));

        // Generate attribute implementations
        let impls = generate_trait_impls(tag, is_svg);
        code.push_str(&impls);
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

            code.push_str(&format!("#[macro_export] macro_rules! {} {{\n", macro_name));
            code.push_str(&format!(
                "    () => {{ $crate::{}::{}(()) }};\n",
                namespace, fn_name
            ));
            code.push_str(&format!(
                "    ($($child:expr),+ $(,)?) => {{ $crate::{}::{}(($($child),+)) }};\n",
                namespace, fn_name
            ));
            code.push_str("}\n");
        }
    }

    code
}

fn generate_trait_impls(tag: &TagDef, is_svg: bool) -> String {
    let mut code = String::new();
    let sys_type = get_web_sys_type(&tag.tag_name, is_svg);
    let name = &tag.struct_name;

    for trait_name in &tag.traits {
        match trait_name.as_str() {
            "FormTag" => code.push_str(&impl_form_attributes(name, &tag.tag_name, &sys_type)),
            "LabelTag" => code.push_str(&impl_label_attributes(name, &sys_type)),
            "AnchorTag" => code.push_str(&impl_anchor_attributes(name, &tag.tag_name, &sys_type)),
            "MediaTag" => code.push_str(&impl_media_attributes(name, &tag.tag_name, &sys_type)),
            "OpenTag" => code.push_str(&impl_open_attributes(name, &sys_type)),
            "TableCellTag" => code.push_str(&impl_table_cell_attributes(name, &sys_type)),
            "TableHeaderTag" => code.push_str(&impl_table_header_attributes(name, &sys_type)),
            _ => {}
        }
    }
    code
}

// --- Specific Implementation Generator ---

fn get_web_sys_type(tag: &str, is_svg: bool) -> String {
    if is_svg {
        return "web_sys::SvgElement".to_string(); // Placeholder for SVG specific types if needed later
    }

    match tag {
        "a" => "web_sys::HtmlAnchorElement",
        "area" => "web_sys::HtmlAreaElement",
        "audio" => "web_sys::HtmlAudioElement",
        "base" => "web_sys::HtmlBaseElement",
        "blockquote" => "web_sys::HtmlQuoteElement",
        "body" => "web_sys::HtmlBodyElement",
        "br" => "web_sys::HtmlBrElement",
        "button" => "web_sys::HtmlButtonElement",
        "canvas" => "web_sys::HtmlCanvasElement",
        "caption" => "web_sys::HtmlTableCaptionElement",
        "col" => "web_sys::HtmlTableColElement",
        "colgroup" => "web_sys::HtmlTableColElement",
        "data" => "web_sys::HtmlDataElement",
        "datalist" => "web_sys::HtmlDataListElement",
        "del" => "web_sys::HtmlModElement",
        "details" => "web_sys::HtmlDetailsElement",
        "dialog" => "web_sys::HtmlDialogElement",
        "div" => "web_sys::HtmlDivElement",
        "dl" => "web_sys::HtmlDListElement",
        "embed" => "web_sys::HtmlEmbedElement",
        "fieldset" => "web_sys::HtmlFieldSetElement",
        "form" => "web_sys::HtmlFormElement",
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => "web_sys::HtmlHeadingElement",
        "head" => "web_sys::HtmlHeadElement",
        "hr" => "web_sys::HtmlHrElement",
        "html" => "web_sys::HtmlHtmlElement",
        "iframe" => "web_sys::HtmlIFrameElement",
        "img" => "web_sys::HtmlImageElement",
        "input" => "web_sys::HtmlInputElement",
        "ins" => "web_sys::HtmlModElement",
        "label" => "web_sys::HtmlLabelElement",
        "legend" => "web_sys::HtmlLegendElement",
        "li" => "web_sys::HtmlLiElement",
        "link" => "web_sys::HtmlLinkElement",
        "map" => "web_sys::HtmlMapElement",
        "menu" => "web_sys::HtmlMenuElement",
        "meta" => "web_sys::HtmlMetaElement",
        "meter" => "web_sys::HtmlMeterElement",
        "object" => "web_sys::HtmlObjectElement",
        "ol" => "web_sys::HtmlOListElement",
        "optgroup" => "web_sys::HtmlOptGroupElement",
        "option" => "web_sys::HtmlOptionElement",
        "output" => "web_sys::HtmlOutputElement",
        "p" => "web_sys::HtmlParagraphElement",
        "param" => "web_sys::HtmlParamElement",
        "picture" => "web_sys::HtmlPictureElement",
        "pre" => "web_sys::HtmlPreElement",
        "progress" => "web_sys::HtmlProgressElement",
        "q" => "web_sys::HtmlQuoteElement",
        "script" => "web_sys::HtmlScriptElement",
        "select" => "web_sys::HtmlSelectElement",
        "slot" => "web_sys::HtmlSlotElement",
        "source" => "web_sys::HtmlSourceElement",
        "span" => "web_sys::HtmlSpanElement",
        "style" => "web_sys::HtmlStyleElement",
        "table" => "web_sys::HtmlTableElement",
        "tbody" => "web_sys::HtmlTableSectionElement",
        "td" => "web_sys::HtmlTableCellElement",
        "template" => "web_sys::HtmlTemplateElement",
        "textarea" => "web_sys::HtmlTextAreaElement",
        "tfoot" => "web_sys::HtmlTableSectionElement",
        "th" => "web_sys::HtmlTableCellElement",
        "thead" => "web_sys::HtmlTableSectionElement",
        "time" => "web_sys::HtmlTimeElement",
        "title" => "web_sys::HtmlTitleElement",
        "tr" => "web_sys::HtmlTableRowElement",
        "track" => "web_sys::HtmlTrackElement",
        "ul" => "web_sys::HtmlUListElement",
        "video" => "web_sys::HtmlVideoElement",
        _ => "web_sys::HtmlElement", // Default fallback
    }
    .to_string()
}

fn impl_form_attributes(struct_name: &str, tag_name: &str, sys_type: &str) -> String {
    let mut methods = String::new();

    // type_ (input, button)
    if tag_name == "input" {
        methods.push_str("    fn type_<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute { let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_type(v)); self }\n");
    } else {
        methods.push_str(
            "    fn type_<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute { self.attr(\"type\", value) }\n",
        );
    }

    // value (input, textarea, select, option, progress, etc)
    if [
        "input", "textarea", "select", "option", "button", "output", "data", "li", "meter",
        "param", "progress",
    ]
    .contains(&tag_name)
    {
        let setter = match tag_name {
            "input" => "el.set_value(v)",
            "textarea" => "el.set_value(v)",
            "select" => "el.set_value(v)",
            "option" => "el.set_value(v)",
            "button" => "el.set_value(v)",
            "output" => "el.set_value(v)",
            "data" => "el.set_value(v)",
            _ => "", // fallback
        };

        if !setter.is_empty() {
            methods.push_str(&format!("    fn value<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| {}); self }}\n", sys_type, setter));
        } else {
            methods.push_str("    fn value<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute { self.prop(\"value\", value) }\n");
        }
    } else {
        methods.push_str("    fn value<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute { self.prop(\"value\", value) }\n");
    }

    // checked (input)
    if tag_name == "input" {
        methods.push_str("    fn checked<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_checked(v)); self }\n");
    } else {
        methods.push_str("    fn checked<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { self.prop(\"checked\", value) }\n");
    }

    // disabled
    if [
        "button", "fieldset", "input", "optgroup", "option", "select", "textarea",
    ]
    .contains(&tag_name)
    {
        methods.push_str(&format!("    fn disabled<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_disabled(v)); self }}\n", sys_type));
    } else {
        methods.push_str("    fn disabled<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { self.prop(\"disabled\", value) }\n");
    }

    // placeholder (input, textarea)
    if tag_name == "input" {
        methods.push_str("    fn placeholder<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute { let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_placeholder(v)); self }\n");
    } else if tag_name == "textarea" {
        methods.push_str("    fn placeholder<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute { let el: web_sys::HtmlTextAreaElement = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_placeholder(v)); self }\n");
    } else {
        methods.push_str("    fn placeholder<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute { self.attr(\"placeholder\", value) }\n");
    }

    // readonly (input, textarea)
    if tag_name == "input" {
        methods.push_str("    fn readonly<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_read_only(v)); self }\n");
    } else if tag_name == "textarea" {
        methods.push_str("    fn readonly<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { let el: web_sys::HtmlTextAreaElement = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_read_only(v)); self }\n");
    } else {
        methods.push_str("    fn readonly<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { self.prop(\"readOnly\", value) }\n");
    }

    // required (input, textarea, select)
    if ["input", "textarea", "select"].contains(&tag_name) {
        methods.push_str(&format!("    fn required<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_required(v)); self }}\n", sys_type));
    } else {
        methods.push_str("    fn required<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { self.prop(\"required\", value) }\n");
    }

    // selected (option)
    if tag_name == "option" {
        methods.push_str("    fn selected<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { let el: web_sys::HtmlOptionElement = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_selected(v)); self }\n");
    } else {
        methods.push_str("    fn selected<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { self.prop(\"selected\", value) }\n");
    }

    // multiple (select, input)
    if tag_name == "input" {
        methods.push_str("    fn multiple<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { let el: web_sys::HtmlInputElement = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_multiple(v)); self }\n");
    } else if tag_name == "select" {
        methods.push_str("    fn multiple<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { let el: web_sys::HtmlSelectElement = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_multiple(v)); self }\n");
    } else {
        methods.push_str("    fn multiple<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute { self.prop(\"multiple\", value) }\n");
    }

    format!(
        "impl FormAttributes for TypedElement<{}> {{\n{}\n}}\n",
        struct_name, methods
    )
}

fn impl_label_attributes(struct_name: &str, sys_type: &str) -> String {
    let mut methods = String::new();
    methods.push_str(&format!("    fn for_<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_html_for(v)); self }}\n", sys_type));

    format!(
        "impl LabelAttributes for TypedElement<{}> {{\n{}\n}}\n",
        struct_name, methods
    )
}

fn impl_anchor_attributes(struct_name: &str, tag_name: &str, sys_type: &str) -> String {
    let mut methods = String::new();
    methods.push_str(&format!("    fn href<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_href(v)); self }}\n", sys_type));
    methods.push_str(&format!("    fn target<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_target(v)); self }}\n", sys_type));
    methods.push_str(&format!("    fn rel<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_rel(v)); self }}\n", sys_type));

    // HtmlLinkElement does not have set_download in web-sys
    if tag_name == "link" {
        methods.push_str("    fn download<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute { self.attr(\"download\", value) }\n");
    } else {
        methods.push_str(&format!("    fn download<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_download(v)); self }}\n", sys_type));
    }

    format!(
        "impl AnchorAttributes for TypedElement<{}> {{\n{}\n}}\n",
        struct_name, methods
    )
}

fn impl_media_attributes(struct_name: &str, tag_name: &str, sys_type: &str) -> String {
    let mut methods = String::new();

    // src
    if [
        "img", "audio", "video", "source", "track", "input", "frame", "iframe",
    ]
    .contains(&tag_name)
    {
        methods.push_str(&format!("    fn src<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_src(v)); self }}\n", sys_type));
    } else {
        methods.push_str(
            "    fn src<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute { self.attr(\"src\", value) }\n",
        );
    }

    methods.push_str("    // width/height passed to attr for flexibility (%, px, auto)\n");

    // autoplay, loop, controls, muted
    if ["audio", "video"].contains(&tag_name) {
        methods.push_str(&format!("    fn autoplay<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_autoplay(v)); self }}\n", sys_type));
        methods.push_str(&format!("    fn controls<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_controls(v)); self }}\n", sys_type));
        methods.push_str(&format!("    fn loop_<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_loop(v)); self }}\n", sys_type));
        methods.push_str(&format!("    fn muted<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_muted(v)); self }}\n", sys_type));
    }

    format!(
        "impl MediaAttributes for TypedElement<{}> {{\n{}\n}}\n",
        struct_name, methods
    )
}

fn impl_open_attributes(struct_name: &str, sys_type: &str) -> String {
    let mut methods = String::new();
    methods.push_str(&format!("    fn open<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyBoolAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyBoolAttribute; value.into_storable().apply_bool(move |v| el.set_open(v)); self }}\n", sys_type));

    format!(
        "impl OpenAttributes for TypedElement<{}> {{\n{}\n}}\n",
        struct_name, methods
    )
}

fn impl_table_cell_attributes(struct_name: &str, sys_type: &str) -> String {
    let mut methods = String::new();
    // colSpan, rowSpan are u32 in web-sys but can be string "2"
    methods.push_str(&format!("    fn colspan<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| {{ if let Ok(n) = v.parse::<u32>() {{ el.set_col_span(n); }} else {{ let _ = el.set_attribute(\"colspan\", v); }} }}); self }}\n", sys_type));
    methods.push_str(&format!("    fn rowspan<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| {{ if let Ok(n) = v.parse::<u32>() {{ el.set_row_span(n); }} else {{ let _ = el.set_attribute(\"rowspan\", v); }} }}); self }}\n", sys_type));

    methods.push_str(&format!("    fn headers<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_headers(v)); self }}\n", sys_type));

    format!(
        "impl TableCellAttributes for TypedElement<{}> {{\n{}\n}}\n",
        struct_name, methods
    )
}

fn impl_table_header_attributes(struct_name: &str, sys_type: &str) -> String {
    let mut methods = String::new();
    methods.push_str(&format!("    fn scope<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_scope(v)); self }}\n", sys_type));
    methods.push_str(&format!("    fn abbr<V>(self, value: V) -> Self where V: IntoStorable, V::Stored: silex_dom::ApplyStringAttribute {{ let el: {} = self.element.dom_element.clone().unchecked_into(); use silex_dom::ApplyStringAttribute; value.into_storable().apply_string(move |v| el.set_abbr(v)); self }}\n", sys_type));

    format!(
        "impl TableHeaderAttributes for TypedElement<{}> {{\n{}\n}}\n",
        struct_name, methods
    )
}
