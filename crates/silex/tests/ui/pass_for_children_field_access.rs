use silex::prelude::*;

#[derive(Clone)]
struct Entry {
    id: u32,
    title: String,
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (keyed_entries, _) = scope.signal(vec![Entry {
            id: 1,
            title: "Keyed entry".to_string(),
        }]);
        let _keyed_view = For(keyed_entries, |entry| entry.id)
            .children(|entry, index, _updater| {
                div(format!("{index}: {}", entry.title))
            })
            .build();

        let (indexed_entries, _) = scope.signal(vec![Entry {
            id: 2,
            title: "Indexed entry".to_string(),
        }]);
        let _indexed_view = Index(indexed_entries)
            .children(|entry, index| div(format!("{index}: {}", entry.title)))
            .build();
    });
}
