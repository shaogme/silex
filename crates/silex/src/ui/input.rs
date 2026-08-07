use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::input;
use silex_macros::{component, tw};

#[component]
pub fn Input<'scope>(
    scope: Scope<'scope>,
    #[prop(into)]
    #[chain(default)]
    value: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    placeholder: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    r#type: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_input: Callback<'scope, String>,
) -> impl View<'scope> {
    let type_source = r#type;

    let input_cls = rx!(scope; {
        let base = tw!(
            "flex h-9 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none selection:bg-primary selection:text-primary-foreground file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-destructive/20 md:text-sm dark:bg-input/30 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let type_val = rx!(scope; {
        let t = $type_source;
        if t.is_empty() { "text".to_string() } else { t.clone() }
    });

    input()
        .attr("data-slot", "input")
        .attr("type", type_val)
        .attr("placeholder", rx!(scope; $placeholder.clone()))
        .prop("value", rx!(scope; $value.clone()))
        .class(input_cls)
        .on(
            event::input,
            move |e: web_sys::InputEvent| -> SilexResult<()> {
                if let Some(target) = e.target()
                    && let Ok(input_el) =
                        wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlInputElement>(target)
                {
                    on_input.invoke(input_el.value())?;
                }
                Ok(())
            },
        )
}
