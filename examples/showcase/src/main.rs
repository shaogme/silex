use silex::prelude::*;

mod advanced;
pub mod basics;
pub mod flow_control;
pub mod routes;

use advanced::{UserSettings, UserSettingsStore};
use routes::{AppRoute, NavBar};

fn main() {
    setup_global_error_handlers();

    // Global State Initialization
    // Convert plain data to Reactive Store
    let store = UserSettingsStore::new(UserSettings {
        theme: "Light".to_string(),
        notifications: true,
        username: "Guest".to_string(),
    });

    // Mount App
    mount_to_body(rx! {
        // Provide Global Store to the entire app tree
        store.provide();

        div![
            // Global Layout Shell
            NavBar(),
            // Root Router
            Router::new().match_route::<AppRoute>(),
        ]
    });
}
