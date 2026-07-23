use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::input;
use silex_macros::{component, tw};

#[component]
pub fn Input(
    #[prop(into)]
    #[chain(default)]
    value: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    placeholder: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    r#type: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    on_input: Callback<String>,
) -> impl View {
    let input_cls = rx!(move || {
        let base = tw!(
            "flex h-9 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none selection:bg-primary selection:text-primary-foreground file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-destructive/20 md:text-sm dark:bg-input/30 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        );
        let extra = class.get();
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let type_val = rx!(move || {
        let t = r#type.get();
        if t.is_empty() {
            "text".to_string()
        } else {
            t
        }
    });

    input()
        .attr("data-slot", "input")
        .attr("type", type_val)
        .attr("placeholder", rx!(move || placeholder.get()))
        .prop("value", rx!(move || value.get()))
        .class(input_cls)
        .on(event::input, move |e: web_sys::InputEvent| {
            if let Some(target) = e.target()
                && let Ok(input_el) = wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlInputElement>(target)
            {
                on_input.call(input_el.value());
            }
        })
}
