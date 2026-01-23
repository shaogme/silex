use silex_core::dom::document;
use wasm_bindgen::JsCast;

/// Injects a CSS string into the document head with a unique ID.
/// This function is idempotent: if a style with the given ID already exists, it does nothing.
///
/// # Arguments
///
/// * `id` - A unique identifier for the style block (e.g. "style-slx-123456").
/// * `content` - The CSS content to inject.
pub fn inject_style(id: &str, content: &str) {
    let doc = document();

    // Check if style already exists to avoid duplication
    if doc.get_element_by_id(id).is_some() {
        return;
    }

    let head = doc.head().expect("No <head> element found in document");

    // Create <style> element
    let style_el = doc
        .create_element("style")
        .expect("Failed to create style element");

    // Set ID and content
    style_el.set_id(id);
    // style_el.set_attribute("type", "text/css").unwrap(); // Optional in HTML5
    style_el.set_inner_html(content);

    // Append to head
    let style_node: rust_wasm::web_sys::Node = style_el.unchecked_into();
    head.append_child(&style_node)
        .expect("Failed to append style to head");
}

// Helper re-export for the macro to use fully qualified names if needed,
// though the macro usually expands to code using `silex::css::inject_style`.
mod rust_wasm {
    pub use web_sys;
}
