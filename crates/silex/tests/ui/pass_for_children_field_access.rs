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
        let (keyed_entries, _) = scope
            .signal(vec![Entry {
                id: 1,
                title: "Keyed entry".to_string(),
            }])
            .expect("keyed entries signal should be created");
        let _keyed_view = For(keyed_entries, |entry| entry.id)
            .error_handler(error_handler)
            .children(|entry, index, _updater| {
                div(format!("{index}: {}", entry.title))
            })
            .build();

        let (indexed_entries, _) = scope
            .signal(vec![Entry {
                id: 2,
                title: "Indexed entry".to_string(),
            }])
            .expect("indexed entries signal should be created");
        let _indexed_view = Index(indexed_entries)
            .error_handler(error_handler)
            .children(|entry, index| div(format!("{index}: {}", entry.title)))
            .build();
    });
}
