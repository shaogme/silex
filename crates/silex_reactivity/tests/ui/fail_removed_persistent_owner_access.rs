use silex_reactivity::PersistentOwnerAccess;

fn main() {
    let _ = std::marker::PhantomData::<PersistentOwnerAccess<'static>>;
}
