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
fn App<'scope>(scope: Scope<'scope>, user: UserStore<'scope>) -> impl View<'scope> {
    div!(
        h1("Silex Store Demo"),
        p("This example demonstrates fine-grained reactivity using the #[store] macro."),
        UserDisplay(scope, user),
        UserEditor(scope, user),
        DebugPanel(scope, user),
    )
    .style(
        "padding: 20px; font-family: sans-serif; max-width: 500px; margin: 0 auto; border: 1px solid #ccc; border-radius: 8px;",
    )
}

#[component]
fn UserDisplay<'scope>(_scope: Scope<'scope>, user: UserStore<'scope>) -> impl View<'scope> {
    div!(
        div!(span("Name: ").style("font-weight: bold;"), span(user.name),),
        div!(span("Age: ").style("font-weight: bold;"), span(user.age),),
        div!(
            span("Email: ").style("font-weight: bold;"),
            span(user.email),
        ),
    )
    .style("background: #f5f5f5; padding: 15px; border-radius: 4px; margin-bottom: 20px;")
}

#[component]
fn UserEditor<'scope>(_scope: Scope<'scope>, user: UserStore<'scope>) -> impl View<'scope> {
    div!(
        div!(
            label("Change Name: "),
            input()
                .type_("text")
                .value(user.name)
                .on_input(move |new_val| {
                    user.name.set(new_val);
                    Ok(())
                }),
        ),
        div!(
            label("Change Age: "),
            button("Increment Age").on_click(move |_| {
                user.age.update(|age| *age += 1);
                Ok(())
            }),
            span("(Only updates Age node)").style("margin-left: 10px; color: #666;"),
        ),
        div!(
            label("Change Email: "),
            input()
                .type_("email")
                .value(user.email)
                .on_input(move |new_val| {
                    user.email.set(new_val);
                    Ok(())
                }),
        ),
    )
    .style("display: flex; flex-direction: column; gap: 10px;")
}

#[component]
fn DebugPanel<'scope>(_scope: Scope<'scope>, user: UserStore<'scope>) -> impl View<'scope> {
    div!(button("Log Current State to Console").on_click(move |_| {
        let current_state = user.snapshot_untracked();
        web_sys::console::log_1(&format!("Current Store State: {:?}", current_state).into());
        Ok(())
    }))
    .style("margin-top: 20px; border-top: 1px dashed #ccc; padding-top: 10px;")
}

/// Mount the Store demo into the conventional `#app` target.
pub fn mount_store() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app")?;
    bootstrap.mount(Runtime::new(), mount_store_view)?;
    bootstrap.into_js_host()
}

/// Mount the Store demo into a caller-provided target node.
pub fn mount_store_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target);
    bootstrap.mount(Runtime::new(), mount_store_view)?;
    bootstrap.into_js_host()
}

fn mount_store_view<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let scope = context.scope();
    let error_handler = scope.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    });
    let user = UserStore::new(
        scope,
        User {
            name: "Alice".to_string(),
            age: 25,
            email: "alice@example.com".to_string(),
        },
    );

    context.mount(App(scope, user), error_handler)
}
