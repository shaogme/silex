use silex_persist::Persistent;

fn main() {
    let _ = Persistent::builder("counter");
}
