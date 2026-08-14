use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;

#[derive(Clone, Debug)]
#[store]
struct User {
    name: String,
    age: i32,
    email: String,
}

#[component]
fn App<'scope, Ctx>(#[context] context: Ctx, user: UserStore<'scope>) -> impl View<'scope> {
    Ok(div!(
        h1("Silex Store Demo"),
        p("This example demonstrates fine-grained reactivity using the #[store] macro."),
        UserDisplay(context, user).build(),
        UserEditor(context, user).build(),
        DebugPanel(context, user).build(),
    )
    .style(
        sty()
            .padding("20px")?
            .font_family("sans-serif")?
            .max_width(px(500))?
            .margin("0 auto")?
            .border("1px solid #ccc")?
            .border_radius(px(8))?,
    ))
}

#[component]
fn UserDisplay<'scope, Ctx>(#[context] context: Ctx, user: UserStore<'scope>) -> impl View<'scope> {
    Ok(div!(
        div!(
            span("Name: ").style(sty().font_weight(FontWeightKeyword::Bold)?),
            span(user.name),
        ),
        div!(
            span("Age: ").style(sty().font_weight(FontWeightKeyword::Bold)?),
            span(user.age),
        ),
        div!(
            span("Email: ").style(sty().font_weight(FontWeightKeyword::Bold)?),
            span(user.email),
        ),
    )
    .style(
        sty()
            .background("#f5f5f5")?
            .padding("15px")?
            .border_radius(px(4))?
            .margin_bottom(px(20))?,
    ))
}

#[component]
fn UserEditor<'scope, Ctx>(#[context] context: Ctx, user: UserStore<'scope>) -> impl View<'scope> {
    Ok(div!(
        div!(
            label("Change Name: "),
            input()
                .type_("text")
                .value(user.name)
                .on_input(move |new_val| {
                    user.name
                        .set(new_val)
                        .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                    Ok(())
                }),
        ),
        div!(
            label("Change Age: "),
            button("Increment Age").on_click(move |_| {
                user.age
                    .update(|age| *age += 1)
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                Ok(())
            }),
            span("(Only updates Age node)").style(sty().margin_left(px(10))?.color(hex("#666"))?),
        ),
        div!(
            label("Change Email: "),
            input()
                .type_("email")
                .value(user.email)
                .on_input(move |new_val| {
                    user.email
                        .set(new_val)
                        .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
                    Ok(())
                }),
        ),
    )
    .style(
        sty()
            .display("flex")?
            .flex_direction(FlexDirectionKeyword::Column)?
            .gap(px(10))?,
    ))
}

#[component]
fn DebugPanel<'scope, Ctx>(#[context] context: Ctx, user: UserStore<'scope>) -> impl View<'scope> {
    Ok(
        div!(button("Log Current State to Console").on_click(move |_| {
            let current_state = user.snapshot_untracked()?;
            web_sys::console::log_1(&format!("Current Store State: {:?}", current_state).into());
            Ok(())
        }))
        .style(
            sty()
                .margin_top(px(20))?
                .border_top("1px dashed #ccc")?
                .padding_top(px(10))?,
        ),
    )
}

/// Mount the Store demo into the conventional `#app` target.
pub fn mount_store() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app", CleanupSink::console())?;
    bootstrap.mount(Runtime::new(), mount_store_view)?;
    bootstrap.into_js_host()
}

/// Mount the Store demo into a caller-provided target node.
pub fn mount_store_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target, CleanupSink::console());
    bootstrap.mount(Runtime::new(), mount_store_view)?;
    bootstrap.into_js_host()
}

fn mount_store_view<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let scope = context.scope();
    let error_handler = scope.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    })?;
    let user = UserStore::new(
        scope,
        User {
            name: "Alice".to_string(),
            age: 25,
            email: "alice@example.com".to_string(),
        },
    )?;

    let silex_context = SilexContext::new(scope, error_handler);
    context.mount(App(silex_context, user).build(), error_handler)
}
