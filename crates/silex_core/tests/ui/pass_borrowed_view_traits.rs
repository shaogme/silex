#![deny(warnings)]

use silex_core::{Constant, RxReadOption, RxReadRef, RxReadTuple2, SilexResult};

fn read_ref<S>(source: &S) -> SilexResult<u32>
where
    S: RxReadRef<u32>,
{
    RxReadRef::with(source, |value| *value)
}

fn read_option<S>(source: &S) -> SilexResult<Option<u32>>
where
    S: RxReadOption<u32>,
{
    RxReadOption::with(source, |value| value.copied())
}

fn read_tuple<S>(source: &S) -> SilexResult<(u32, String)>
where
    S: RxReadTuple2<u32, String>,
{
    RxReadTuple2::with(source, |(first, second)| (*first, second.clone()))
}

fn main() -> SilexResult<()> {
    let ref_source = Constant::new(7_u32);
    assert_eq!(read_ref(&ref_source)?, 7);
    assert_eq!(RxReadRef::with_untracked(&ref_source, |value| *value)?, 7);

    let value = 8_u32;
    let non_static_source = Constant::new(value);
    assert_eq!(read_ref(&non_static_source)?, 8);

    let option_source = Constant::new(Some(9_u32));
    assert_eq!(read_option(&option_source)?, Some(9));
    assert_eq!(
        RxReadOption::with_untracked(&option_source, |value| value.is_some())?,
        true
    );

    let tuple_source = (Constant::new(10_u32), Constant::new(String::from("view")));
    assert_eq!(read_tuple(&tuple_source)?, (10, String::from("view")));
    assert_eq!(
        RxReadTuple2::with_untracked(&tuple_source, |(first, second)| (*first, second.len()))?,
        (10, 4)
    );

    Ok(())
}
