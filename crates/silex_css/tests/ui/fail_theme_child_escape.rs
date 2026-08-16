use silex_core::Runtime;
use silex_css::prelude::*;
use std::fmt::{Display, Formatter};

#[derive(Clone)]
struct Theme;

impl Display for Theme {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("--color: red;")
    }
}

impl ThemeType for Theme {}

impl ThemeToCss for Theme {
    fn get_variable_values(&self) -> Vec<String> {
        vec![String::from("red")]
    }

    fn get_variable_names() -> &'static [&'static str] {
        &["--color"]
    }
}

fn main() {
    let mut runtime = Runtime::new();
    let theme = runtime.with_transient(|owner| {
        let (value, _) = owner
            .signal(Theme)
            .expect("signal should initialize");
        theme_variables(value)
    });
    let _ = theme;
}
