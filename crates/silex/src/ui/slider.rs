use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{FormAttributes, div, input};
use silex_macros::{component, tw};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::JsCast;

fn calculate_slider_value(
    e: &web_sys::PointerEvent,
    track_el: &web_sys::Element,
    is_vertical: bool,
    min: f64,
    max: f64,
    step: f64,
) -> f64 {
    let rect = track_el.get_bounding_client_rect();
    let pct = if is_vertical {
        let height = rect.height();
        if height > 0.0 {
            let client_y = e.client_y() as f64;
            let bottom = rect.bottom();
            ((bottom - client_y) / height).clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        let width = rect.width();
        if width > 0.0 {
            let client_x = e.client_x() as f64;
            let left = rect.left();
            ((client_x - left) / width).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    let step_safe = if step <= 0.0 { 1.0 } else { step };
    let raw_val = min + pct * (max - min);
    let stepped_val = (raw_val / step_safe).round() * step_safe;
    let min_bound = min.min(max);
    let max_bound = min.max(max);
    stepped_val.clamp(min_bound, max_bound)
}

#[component]
pub fn Slider<'scope>(
    scope: Scope<'scope>,
    #[prop(into)]
    #[chain(default)]
    value: Signal<'scope, f64>,
    #[prop(into)]
    #[chain(default)]
    min: Signal<'scope, f64>,
    #[prop(into)]
    #[chain(default)]
    max: Signal<'scope, f64>,
    #[prop(into)]
    #[chain(default)]
    step: Signal<'scope, f64>,
    #[prop(into)]
    #[chain(default)]
    orientation: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    disabled: Signal<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_change: Callback<'scope, f64>,
) -> impl View<'scope> {
    let min_val = rx!(scope; *$min);

    let max_val = rx!(scope; {
        let mn = *$min;
        let mx = *$max;
        if mx <= mn {
            if mn == 0.0 && mx == 0.0 {
                100.0
            } else {
                mn + 100.0
            }
        } else {
            mx
        }
    });

    let step_val = rx!(scope; {
        let s = *$step;
        if s <= 0.0 { 1.0 } else { s }
    });

    let is_vertical = rx!(scope; $orientation.as_str() == "vertical");
    let orient = rx!(scope; if *$is_vertical {
        "vertical"
    } else {
        "horizontal"
    });

    let root_cls = rx!(scope; {
        let base = tw!(
            "relative flex w-full touch-none items-center select-none data-[disabled]:opacity-50 data-[orientation=vertical]:h-full data-[orientation=vertical]:min-h-44 data-[orientation=vertical]:w-auto data-[orientation=vertical]:flex-col"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let pct = rx!(scope; {
        let v = *$value;
        let mn = *$min_val;
        let mx = *$max_val;
        if mx <= mn {
            0.0
        } else {
            ((v - mn) / (mx - mn) * 100.0).clamp(0.0, 100.0)
        }
    });

    let range_style = rx!(scope; {
        let p = *$pct;
        if *$is_vertical {
            format!("height: {}%;", p)
        } else {
            format!("width: {}%;", p)
        }
    });

    let thumb_style = rx!(scope; {
        let p = *$pct;
        if *$is_vertical {
            format!("bottom: {}%;", p)
        } else {
            format!("left: {}%;", p)
        }
    });

    let input_val_str = rx!(scope; $value.to_string());
    let is_dragging = Rc::new(Cell::new(false));

    let handle_down = {
        let is_dragging = is_dragging.clone();
        move |e: web_sys::PointerEvent| {
            if disabled.get() {
                return;
            }
            if let Some(target) = e.current_target()
                && let Ok(el) = target.dyn_into::<web_sys::Element>()
                && el.set_pointer_capture(e.pointer_id()).is_ok()
            {
                is_dragging.set(true);
                let new_val = calculate_slider_value(
                    &e,
                    &el,
                    is_vertical.get(),
                    min_val.get(),
                    max_val.get(),
                    step_val.get(),
                );
                let _ = on_change.invoke(new_val);
            }
        }
    };

    let handle_move = {
        let is_dragging = is_dragging.clone();
        move |e: web_sys::PointerEvent| {
            if is_dragging.get()
                && let Some(target) = e.current_target()
                && let Ok(el) = target.dyn_into::<web_sys::Element>()
            {
                let new_val = calculate_slider_value(
                    &e,
                    &el,
                    is_vertical.get(),
                    min_val.get(),
                    max_val.get(),
                    step_val.get(),
                );
                let _ = on_change.invoke(new_val);
            }
        }
    };

    let handle_up = {
        let is_dragging = is_dragging.clone();
        move |e: web_sys::PointerEvent| {
            if is_dragging.get() {
                is_dragging.set(false);
                if let Some(target) = e.current_target()
                    && let Ok(el) = target.dyn_into::<web_sys::Element>()
                {
                    let _ = el.release_pointer_capture(e.pointer_id());
                }
            }
        }
    };

    let handle_input = move |v: String| {
        if let Ok(num) = v.parse::<f64>() {
            let _ = on_change.invoke(num);
        }
    };

    div(view_chain!(
        // Hidden Range Input for Accessibility & Keyboard arrows
        input()
            .type_("range")
            .value(input_val_str)
            .min(min_val)
            .max(max_val)
            .step(step_val)
            .disabled(disabled)
            .class(tw!("sr-only absolute opacity-0 pointer-events-none"))
            .on_input(handle_input)
            .on_change(handle_input),
        // Visual Track
        div(view_chain!(
            // Visual Range Fill
            div(())
                .attr("data-slot", "slider-range")
                .attr("data-orientation", orient)
                .class(tw!("absolute bg-primary data-[orientation=horizontal]:h-full data-[orientation=vertical]:w-full"))
                .style(range_style)
        ))
        .attr("data-slot", "slider-track")
        .attr("data-orientation", orient)
        .class(tw!("relative grow overflow-hidden rounded-full bg-muted data-[orientation=horizontal]:h-1.5 data-[orientation=horizontal]:w-full data-[orientation=vertical]:h-full data-[orientation=vertical]:w-1.5 cursor-pointer")),
        // Visual Thumb
        div(())
            .attr("data-slot", "slider-thumb")
            .attr("data-orientation", orient)
            .class(tw!("absolute top-1/2 -translate-x-1/2 -translate-y-1/2 size-4 shrink-0 rounded-full border border-primary bg-white shadow-sm ring-ring/50 transition-[color,box-shadow] hover:ring-4 focus-visible:ring-4 focus-visible:outline-hidden disabled:pointer-events-none disabled:opacity-50 cursor-pointer"))
            .style(thumb_style)
    ))
    .attr("data-slot", "slider")
    .attr("data-orientation", orient)
    .attr("data-disabled", rx!(scope; if *$disabled { Some("") } else { None }))
    .class(root_cls)
    .on_pointer_down(handle_down)
    .on_pointer_move(handle_move)
    .on_pointer_up(handle_up.clone())
    .on_pointer_cancel(handle_up)
}
