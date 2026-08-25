use silex_core::{
    AppHostError, BootstrapError, DomCleanupReport, MountError, SilexError, SilexErrorKind,
};

fn main() {
    let report = DomCleanupReport::new();
    let mount = MountError::poisoned(SilexError::fatal(SilexErrorKind::Framework(String::new())));
    let host = AppHostError::Mount(Box::new(mount));
    let bootstrap = BootstrapError::from(host);
    let _ = (report, bootstrap);
}
