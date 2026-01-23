use crate::SilexError;
use silex_core::reactivity::{Accessor, create_effect};
use silex_dom::View;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::Node;

/// Switch/Match 组件：多路分支渲染
///
/// # Example
/// ```rust
/// use silex::prelude::*;
/// let (count, set_count) = create_signal(0);
///
/// Switch::new(count, || "Default View")
///     .case(0, || "Zero")
///     .case(1, || "One");
/// ```
#[derive(Clone)]
pub struct Switch<Source, T, V> {
    source: Source,
    cases: Vec<(T, Rc<dyn Fn() -> V>)>,
    fallback: Rc<dyn Fn() -> V>,
    _marker: std::marker::PhantomData<V>,
}

impl<Source, T, V> Switch<Source, T, V>
where
    Source: Accessor<T> + 'static,
    T: PartialEq + Clone + 'static,
    V: View + 'static,
{
    pub fn new(source: Source, fallback: impl Fn() -> V + 'static) -> Self {
        Self {
            source,
            cases: Vec::new(),
            fallback: Rc::new(fallback),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn case(mut self, value: T, view_fn: impl Fn() -> V + 'static) -> Self {
        self.cases.push((value, Rc::new(view_fn)));
        self
    }
}

impl<Source, T, V> View for Switch<Source, T, V>
where
    Source: Accessor<T> + 'static,
    T: PartialEq + Clone + 'static,
    V: View + 'static,
{
    fn mount(self, parent: &Node) {
        let document = silex_dom::document();
        let start_marker = document.create_comment("switch-start");
        let start_node: Node = start_marker.into();
        if let Err(e) = parent.append_child(&start_node) {
            silex_core::error::handle_error(SilexError::from(e));
            return;
        }

        let end_marker = document.create_comment("switch-end");
        let end_node: Node = end_marker.into();
        if let Err(e) = parent.append_child(&end_node) {
            silex_core::error::handle_error(SilexError::from(e));
            return;
        }

        let source = self.source;
        let cases = Rc::new(self.cases);
        let fallback = self.fallback;

        let prev_index = Rc::new(RefCell::new(None::<isize>));

        create_effect(move || {
            let val = source.value();
            let mut found_idx = -1;
            let mut view_fn = fallback.clone();

            for (i, (case_val, case_view)) in cases.iter().enumerate() {
                if *case_val == val {
                    found_idx = i as isize;
                    view_fn = case_view.clone();
                    break;
                }
            }

            let mut prev = prev_index.borrow_mut();
            if *prev == Some(found_idx) {
                return;
            }
            *prev = Some(found_idx);

            // Cleanup
            if let Some(parent) = start_node.parent_node() {
                while let Some(sibling) = start_node.next_sibling() {
                    if sibling == end_node {
                        break;
                    }
                    let _ = parent.remove_child(&sibling);
                }
            }

            // Render
            let fragment = document.create_document_fragment();
            let fragment_node: Node = fragment.clone().into();

            // Handle panic in view generation/render to avoid crash loop?
            // "view_fn().mount()" should be safe-ish user code.
            view_fn().mount(&fragment_node);

            if let Some(parent) = end_node.parent_node() {
                let _ = parent.insert_before(&fragment_node, Some(&end_node));
            }
        });
    }
}
