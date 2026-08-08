use silex_dom::mounted::MountedApp;

fn use_after_dispose(app: MountedApp) {
    let _ = app.dispose();
    let _ = app.is_active();
}

fn main() {}
