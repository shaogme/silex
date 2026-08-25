// comment-only line

const SAME_LINE: u8 = 1; // trailing comment

#[cfg(feature = "fixture")]
fn production_cfg() {
    let _value = 2;
}

#[cfg(any(test, feature = "fixture"))]
fn mixed_cfg() {
    let _value = 3;
}

#[cfg(all(test, feature = "fixture"))]
fn cfg_test_only() {
    let _value = 4;
}

#[cfg(test)]
mod inline_tests {
    #[test]
    fn ignored() {
        let _value = 5;
    }
}

#[test]
fn attribute_test() {}
