use silex_css::prelude::*;

fn main() {
    let style = Style::new()
        .raw("--color", "red")
        .expect("style should build");
    let _ = style.into_rx();
}
