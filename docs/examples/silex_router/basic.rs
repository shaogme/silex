use silex_router::{PathTail, RoutePath, dom::view::AnyView, macros::router};
use std::error::Error;

router! {
    pub enum AppRoute {
        Home => "/",
        User { id: u32 } => "/users/:id",
        Files { rest: PathTail } => "/files/*rest",
        NotFound => "/*",
    }
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let user_path = AppRoute::User { id: 7 }.path()?;
    assert_eq!(user_path.as_str(), "/users/7");

    let local_path = RoutePath::new("/files/a%2Fb")?;
    assert_eq!(local_path.as_str(), "/files/a%2Fb");

    assert!(matches!(
        AppRoute::match_path("/users/7")?,
        Some(AppRoute::User { id: 7 })
    ));
    assert!(matches!(
        AppRoute::match_path("/users/not-a-number")?,
        Some(AppRoute::NotFound)
    ));

    let table = AppRoute::table(|route, _ctx| match route {
        AppRoute::Home => AnyView::from("home"),
        AppRoute::User { id } => AnyView::from(id.to_string()),
        AppRoute::Files { rest } => AnyView::from(rest.into_inner()),
        AppRoute::NotFound => AnyView::from("not found"),
    })?;
    assert!(table.match_path("/files/docs/reference").is_some());

    Ok(())
}
