use silex::prelude::*;

#[derive(Clone)]
struct Entry {
    id: u32,
    title: String,
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(scope, error_handler);
        let (keyed_entries, _) = scope
            .signal(vec![Entry {
                id: 1,
                title: "Keyed entry".to_string(),
            }])
            .expect("keyed entries signal should be created");
        let _keyed_view = For(ctx, keyed_entries, |entry| entry.id)
            .children(|entry, index| {
                div(format!("{index}: {}", entry.title))
            })
            .build();

        let (indexed_entries, _) = scope
            .signal(vec![Entry {
                id: 2,
                title: "Indexed entry".to_string(),
            }])
            .expect("indexed entries signal should be created");
        let _indexed_view = Index(ctx, indexed_entries)
            .children(|entry, index| div(format!("{index}: {}", entry.title)))
            .build();

        let (stateful_entries, _) = scope
            .signal(vec![Entry {
                id: 3,
                title: "Stateful entry".to_string(),
            }])
            .expect("stateful entries signal should be created");
        let _stateful_view = ForStateful(ctx, stateful_entries, |entry| entry.id)
            .children(|entry, index, updater| {
                let _ = updater;
                div(format!("{index}: {}", entry.title))
            })
            .build();
    });
}
