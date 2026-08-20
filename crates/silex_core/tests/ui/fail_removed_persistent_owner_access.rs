use silex_core::PersistentOwnerAccess;

fn main() {
    let _ = std::marker::PhantomData::<PersistentOwnerAccess<'static>>;
}
