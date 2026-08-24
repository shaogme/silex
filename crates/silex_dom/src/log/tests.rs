use super::*;

#[test]
fn logger_functions_and_macros_are_available_on_native_targets() {
    console_log("test console_log");
    console_warn("test console_warn");
    console_error("test console_error");
    console_debug_log("test console_debug_log");
    console_debug_warn("test console_debug_warn");
    console_debug_error("test console_debug_error");

    log!("test log macro: {}", 1);
    warn!("test warn macro: {}", 2);
    error!("test error macro: {}", 3);
    debug_log!("test debug log macro");
    debug_warn!("test debug warn macro");
    debug_error!("test debug error macro");
}
