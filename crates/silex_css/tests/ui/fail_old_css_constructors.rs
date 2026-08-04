use silex_css::prelude::*;

fn main() {
    let style = Style::new().raw("--color", "red");
    let _ = style.into_rx();
}
