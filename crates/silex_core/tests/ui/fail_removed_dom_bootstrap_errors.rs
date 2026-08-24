use silex_core::{AppHostError, BootstrapError, CleanupReport, MountError};

fn main() {
    let _ = (AppHostError::AlreadyMounted, BootstrapError::Lifecycle(String::new()));
    let _ = CleanupReport::new();
    let _ = MountError::poisoned(silex_core::SilexError::fatal(
        silex_core::SilexErrorKind::Framework(String::new()),
    ));
}
