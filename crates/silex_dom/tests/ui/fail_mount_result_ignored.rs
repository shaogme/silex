#![deny(unused_must_use)]

use silex_core::SilexResult;

trait FallibleMount {
    fn mount(&self) -> SilexResult<()>;
}

struct TestView;

impl FallibleMount for TestView {
    fn mount(&self) -> SilexResult<()> {
        Ok(())
    }
}

fn main() {
    TestView.mount();
}
