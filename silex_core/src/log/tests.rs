use super::*;

#[test]
fn test_console_log() {
    console_log("test console_log");
    log!("test log macro");
    log!("test log macro with args: {}", 1);
}

#[test]
fn test_console_warn() {
    console_warn("test console_warn");
    warn!("test warn macro");
    warn!("test warn macro with args: {}", 2);
}

#[test]
fn test_console_error() {
    console_error("test console_error");
    error!("test error macro");
    error!("test error macro with args: {}", 3);
}

#[test]
fn test_debug_logs() {
    console_debug_log("test console_debug_log");
    debug_log!("test debug_log macro");

    console_debug_warn("test console_debug_warn");
    debug_warn!("test debug_warn macro");

    console_debug_error("test console_debug_error");
    debug_error!("test debug_error macro");
}
