#![deny(warnings)]

use silex_core::{
    ErrorHandlerToken, Mutation, OwnerAccess, Resource, RxReadOption, SilexResult,
};

struct NonClone(u32);

#[allow(dead_code)]
fn resource_view<'owner>(
    resource: &Resource<'owner, NonClone, String>,
) -> SilexResult<bool> {
    RxReadOption::with(resource, |value| value.is_some())
}

#[allow(dead_code)]
fn resource_non_clone_api<'owner>(
    resource: &Resource<'owner, NonClone, String>,
    owner: OwnerAccess<'owner>,
    handler: ErrorHandlerToken<'owner>,
) -> SilexResult<()> {
    let _ = resource.state();
    resource.refetch()?;
    resource.update(|value| value.0 += 1)?;
    resource.set(NonClone(2))?;
    let _ = resource.loading()?;
    let _ = resource.map(owner, |value| value.is_some(), handler)?;
    Ok(())
}

#[allow(dead_code)]
fn mutation_view<'owner>(
    mutation: &Mutation<'owner, (), NonClone, String>,
) -> SilexResult<bool> {
    RxReadOption::with(mutation, |value| value.is_some())
}

fn main() {}
