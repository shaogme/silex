use crate::{
    ErrorHandlerInput, OwnerAccess, Rx, SilexResult,
    reactivity::ReactiveSource,
    traits::{RxRead, RxReadRef},
};

fn compare<'owner, A, B, F, H>(
    owner: OwnerAccess<'owner>,
    left: A,
    right: B,
    compare: F,
    error_handler: H,
) -> SilexResult<Rx<'owner, bool>>
where
    A: ReactiveSource<'owner> + 'owner,
    B: ReactiveSource<'owner> + 'owner,
    A::Owned: Sized + PartialEq + 'owner,
    B::Owned: Sized + PartialEq + 'owner,
    F: Fn(&A::Owned, &B::Owned) -> bool + 'owner,
    H: ErrorHandlerInput<'owner>,
{
    let error_handler = error_handler.handler_ref();
    let left = left.into_promotion_plan();
    let right = right.into_promotion_plan();
    let left = left.materialize(owner, error_handler)?;
    let right = right.materialize(owner, error_handler)?;
    owner
        .computed(
            move || left.with(|left| right.with(|right| compare(left, right)))?,
            error_handler,
        )
        .map(crate::Computed::into_rx)
}

pub trait ReactivePartialEq: RxRead + Clone {
    fn equals<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialEq + Sized + 'owner,
        H: ErrorHandlerInput<'owner>;

    fn not_equals<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialEq + Sized + 'owner,
        H: ErrorHandlerInput<'owner>;
}

impl<S> ReactivePartialEq for S
where
    S: RxRead + Clone,
{
    fn equals<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialEq + Sized + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        compare(
            owner,
            self.clone(),
            other,
            |left, right| left == right,
            error_handler,
        )
    }

    fn not_equals<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialEq + Sized + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        compare(
            owner,
            self.clone(),
            other,
            |left, right| left != right,
            error_handler,
        )
    }
}

pub trait ReactivePartialOrd: RxRead + Clone {
    fn greater_than<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialOrd + Sized + 'owner,
        H: ErrorHandlerInput<'owner>;

    fn less_than<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialOrd + Sized + 'owner,
        H: ErrorHandlerInput<'owner>;

    fn greater_than_or_equals<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialOrd + Sized + 'owner,
        H: ErrorHandlerInput<'owner>;

    fn less_than_or_equals<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialOrd + Sized + 'owner,
        H: ErrorHandlerInput<'owner>;
}

impl<S> ReactivePartialOrd for S
where
    S: RxRead + Clone,
{
    fn greater_than<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialOrd + Sized + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        compare(
            owner,
            self.clone(),
            other,
            |left, right| left > right,
            error_handler,
        )
    }

    fn less_than<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialOrd + Sized + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        compare(
            owner,
            self.clone(),
            other,
            |left, right| left < right,
            error_handler,
        )
    }

    fn greater_than_or_equals<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialOrd + Sized + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        compare(
            owner,
            self.clone(),
            other,
            |left, right| left >= right,
            error_handler,
        )
    }

    fn less_than_or_equals<'owner, O, H>(
        &self,
        owner: OwnerAccess<'owner>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        O: ReactiveSource<'owner, Owned = Self::Owned> + 'owner,
        Self::Owned: PartialOrd + Sized + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        compare(
            owner,
            self.clone(),
            other,
            |left, right| left <= right,
            error_handler,
        )
    }
}
