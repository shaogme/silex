use silex_core::reactivity::{Memo, Signal};
use silex_net::HttpClient;

fn main() {
    let _ = HttpClient::get("https://example.test");
    let _ = Signal::pair(1_i32);
    let _ = Memo::new(|_| 1_i32);
}
