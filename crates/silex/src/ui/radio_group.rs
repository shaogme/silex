use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{DataAttributes, FormAttributes, button, div, input, span};
use silex_macros::{component, tw};

#[component]
pub fn RadioGroup<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    orientation: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    disabled: Signal<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let orient = rx!(scope; error_handler; {
        if $orientation.as_str() == "horizontal" {
            "horizontal"
        } else {
            "vertical"
        }
    });

    let group_cls = rx!(scope; error_handler; {
        let base = tw!(
            "grid gap-3 data-[orientation=horizontal]:flex data-[orientation=horizontal]:flex-row data-[orientation=horizontal]:items-center data-[disabled]:opacity-50 data-[disabled]:pointer-events-none"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    Ok(div(children)
        .data_slot("radio-group")
        .data_orientation(orient)
        .data_disabled(rx!(scope; error_handler; *$disabled))
        .class(group_cls))
}

#[component]
pub fn RadioGroupItem<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    value: &'static str,
    #[prop(into)]
    #[chain(default)]
    selected_value: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    name: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    disabled: Signal<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    required: Signal<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_select: Callback<'scope, &'static str>,
    #[prop(into)]
    #[chain(default)]
    on_change: Callback<'scope, String>,
) -> impl View<'scope> {
    let is_checked = rx!(scope; error_handler; $selected_value.as_str() == value);

    let item_cls = rx!(scope; error_handler; {
        let base = tw!(
            "relative aspect-square size-4 shrink-0 rounded-full border border-slate-300 dark:border-slate-700 text-primary shadow-xs transition-all outline-none cursor-pointer flex items-center justify-center bg-white dark:bg-slate-950 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:border-primary data-[state=checked]:text-primary"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let indicator_cls = rx!(scope; error_handler; {
        if *$is_checked {
            tw!("size-2 rounded-full bg-primary transition-transform duration-150 scale-100")
        } else {
            tw!(
                "size-2 rounded-full bg-primary transition-transform duration-150 scale-0 opacity-0"
            )
        }
    });

    let handle_click = move |_| -> SilexResult<()> {
        if !disabled.get()? {
            on_select.invoke(value)?;
            on_change.invoke(value.to_string())?;
        }
        Ok(())
    };

    Ok(button(chain!(
        // Hidden native radio input for Accessibility & Form submission (referencing slider.rs pattern)
        input()
            .type_("radio")
            .name(name)
            .value(value)
            .checked(is_checked)
            .disabled(disabled)
            .required(required)
            .class(tw!("sr-only absolute opacity-0 pointer-events-none")),
        // Visual Radio Indicator Dot
        span(())
            .data_slot("radio-group-indicator")
            .class(indicator_cls)
    ))
    .data_slot("radio-group-item")
    .data_value(value)
    .data_state(rx!(scope; error_handler; if *$is_checked {
        "checked"
    } else {
        "unchecked"
    }))
    .data_disabled(rx!(scope; error_handler; *$disabled))
    .aria_checked(rx!(scope; error_handler; if *$is_checked { "true" } else { "false" }))
    .disabled(disabled)
    .class(item_cls)
    .on_click(handle_click))
}
