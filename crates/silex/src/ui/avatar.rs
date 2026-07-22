use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{MediaAttributes, div, img, span};
use silex_macros::{component, tw, tw_variants};

#[component]
pub fn Avatar(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    size: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let avatar_variants = tw_variants! {
        base: "group/avatar relative flex shrink-0 overflow-hidden rounded-full select-none",
        variants: {
            size: {
                default: "size-8",
                sm: "size-6",
                lg: "size-10"
            }
        },
        default_variants: {
            size: "default"
        }
    };

    let cls = rx!(move || {
        let sz = size.get();
        let base_cls = avatar_variants.get(if sz.is_empty() {
            "default"
        } else {
            sz.as_str()
        });
        let extra = class.get();
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    let data_sz = rx!(move || {
        let sz = size.get();
        if sz.is_empty() {
            "default".to_string()
        } else {
            sz
        }
    });

    div(children)
        .class(cls)
        .attr("data-slot", "avatar")
        .attr("data-size", data_sz)
}

#[component]
pub fn AvatarImage(
    #[prop(into)]
    #[chain(default)]
    src: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    alt: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let cls = rx!(move || {
        let base_cls = tw!("aspect-square size-full object-cover");
        let extra = class.get();
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    img()
        .src(src)
        .alt(alt)
        .class(cls)
        .attr("data-slot", "avatar-image")
}

#[component]
pub fn AvatarFallback(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    variant: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let avatar_fallback_variants = tw_variants! {
        base: "flex size-full items-center justify-center rounded-full text-sm group-data-[size=sm]/avatar:text-xs",
        variants: {
            variant: {
                default: "bg-muted text-muted-foreground",
                indigo: "bg-indigo-600 text-white",
                emerald: "bg-emerald-600 text-white"
            }
        },
        default_variants: {
            variant: "default"
        }
    };

    let cls = rx!(move || {
        let v = variant.get();
        let base_cls =
            avatar_fallback_variants.get(if v.is_empty() { "default" } else { v.as_str() });
        let extra = class.get();
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    span(children)
        .class(cls)
        .attr("data-slot", "avatar-fallback")
}

#[component]
pub fn AvatarBadge(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let cls = rx!(move || {
        let base_cls = tw!(
            "absolute right-0 bottom-0 z-10 inline-flex items-center justify-center rounded-full bg-primary text-primary-foreground ring-2 ring-background select-none group-data-[size=sm]/avatar:size-2 group-data-[size=default]/avatar:size-2.5 group-data-[size=lg]/avatar:size-3"
        );
        let extra = class.get();
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    span(children).class(cls).attr("data-slot", "avatar-badge")
}

#[component]
pub fn AvatarGroup(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let cls = rx!(move || {
        let base_cls = tw!(
            "group/avatar-group flex -space-x-2 *:data-[slot=avatar]:ring-2 *:data-[slot=avatar]:ring-background"
        );
        let extra = class.get();
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    div(children).class(cls).attr("data-slot", "avatar-group")
}

#[component]
pub fn AvatarGroupCount(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let cls = rx!(move || {
        let base_cls = tw!(
            "relative flex size-8 shrink-0 items-center justify-center rounded-full bg-muted text-sm text-muted-foreground ring-2 ring-background group-has-data-[size=lg]/avatar-group:size-10 group-has-data-[size=sm]/avatar-group:size-6"
        );
        let extra = class.get();
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    div(children)
        .class(cls)
        .attr("data-slot", "avatar-group-count")
}
