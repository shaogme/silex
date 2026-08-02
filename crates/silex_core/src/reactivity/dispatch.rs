/// Report an access to a value that is no longer available.
#[cold]
pub fn report_disposed(debug_name: Option<String>) -> ! {
    match debug_name {
        Some(name) => panic!("reactive value {name:?} was disposed"),
        None => panic!("reactive value was disposed"),
    }
}
