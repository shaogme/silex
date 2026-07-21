pub struct AdaptiveWrapper<'a, T>(pub &'a T);

impl<'a, T: Clone> AdaptiveWrapper<'a, T> {
    pub fn maybe_clone(&self) -> Option<T> {
        Some(self.0.clone())
    }
}

pub trait AdaptiveFallback {
    type Value;
    fn maybe_clone(&self) -> Option<Self::Value>;
}

impl<'a, T> AdaptiveFallback for AdaptiveWrapper<'a, T> {
    type Value = T;
    fn maybe_clone(&self) -> Option<T> {
        None
    }
}
