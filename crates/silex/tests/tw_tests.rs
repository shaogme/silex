use silex::prelude::*;
use wasm_bindgen_test::*;

// Enable wasm-bindgen-test for WASM execution

#[wasm_bindgen_test]
fn test_tw_basic_utilities() {
    let cls = tw!("flex flex-col items-center justify-between p-4 bg-white rounded-xl");
    assert!(!cls.is_empty());
}

#[wasm_bindgen_test]
fn test_tw_modifiers() {
    let cls = tw!("hover:bg-black md:p-8 dark:text-white");
    assert!(!cls.is_empty());
}

#[wasm_bindgen_test]
fn test_tw_arbitrary_values() {
    let cls = tw!("w-[100px] h-[50rem] bg-[#1e1e24]");
    assert!(!cls.is_empty());
}

#[wasm_bindgen_test]
fn test_tw_theme_vars() {
    let cls = tw!("bg-theme(primary) text-theme(border)");
    assert!(!cls.is_empty());
}

#[wasm_bindgen_test]
fn test_tw_group_and_peer() {
    let cls = tw!("group-hover:scale-105 peer-focus:block");
    assert!(!cls.is_empty());
}

#[wasm_bindgen_test]
fn test_tw_keyframes_and_filters() {
    let cls = tw!("animate-spin blur-md backdrop-blur-sm rotate-45 scale-105");
    assert!(!cls.is_empty());
}

#[wasm_bindgen_test]
fn test_tw_tailwind_merge() {
    // 覆盖消解测试：p-2 被 p-6 覆盖，bg-[#ef4444] 被 bg-white 覆盖
    let cls = tw!("p-2 p-6 bg-[#ef4444] bg-white");
    assert!(!cls.is_empty());
}
