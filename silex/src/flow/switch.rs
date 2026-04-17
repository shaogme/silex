use silex_core::reactivity::Effect;
use silex_core::traits::RxGet;
use silex_dom::prelude::View;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::Node;

type BoxedViewFactory<V> = Rc<dyn crate::flow::ViewFactory<View = V>>;
type Case<T, V> = (T, BoxedViewFactory<V>);

/// Switch/Match 组件：多路分支渲染
///
/// # Example
/// ```rust
/// use silex::prelude::*;
/// let (count, set_count) = Signal::new(0);
///
/// Switch::new(count, rx!("Default View"))
///     .case(0, rx!("Zero"))
///     .case(1, rx!("One"));
/// ```
#[derive(Clone)]
pub struct Switch<Source, T, V> {
    source: Source,
    cases: Vec<Case<T, V>>,
    fallback: BoxedViewFactory<V>,
    _marker: std::marker::PhantomData<V>,
}

impl<Source, T, V> Switch<Source, T, V>
where
    Source: RxGet<Value = T> + 'static,
    T: PartialEq + Clone + 'static,
    V: View + 'static,
{
    pub fn new(
        source: Source,
        fallback: impl crate::flow::ViewFactory<View = V> + 'static,
    ) -> Self {
        Self {
            source,
            cases: Vec::new(),
            fallback: Rc::new(fallback),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn case(
        mut self,
        value: T,
        view_fn: impl crate::flow::ViewFactory<View = V> + 'static,
    ) -> Self {
        self.cases.push((value, Rc::new(view_fn)));
        self
    }
}

impl<Source, T, V> View for Switch<Source, T, V>
where
    Source: RxGet<Value = T> + Clone + 'static,
    T: PartialEq + Clone + 'static,
    V: View + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_switch_internal(
            self.source,
            Rc::new(self.cases),
            self.fallback,
            parent,
            attrs,
        );
    }

    fn mount_ref(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_switch_internal(
            self.source.clone(),
            Rc::new(self.cases.clone()),
            self.fallback.clone(),
            parent,
            attrs,
        );
    }
}

fn mount_switch_internal<Source, T, V>(
    source: Source,
    cases: Rc<Vec<Case<T, V>>>,
    fallback: BoxedViewFactory<V>,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    Source: RxGet<Value = T> + 'static,
    T: PartialEq + Clone + 'static,
    V: View + 'static,
{
    let document = silex_dom::document();
    let start_node: Node = document.create_comment("switch-start").into();
    let _ = parent.append_child(&start_node);

    let end_node: Node = document.create_comment("switch-end").into();
    let _ = parent.append_child(&end_node);

    let prev_index = Rc::new(RefCell::new(None::<isize>));

    Effect::new(move |_| {
        let val = source.get();
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

        if let Some(parent) = start_node.parent_node() {
            while let Some(sibling) = start_node.next_sibling() {
                if sibling == end_node {
                    break;
                }
                let _ = parent.remove_child(&sibling);
            }
        }

        let fragment_node: Node = document.create_document_fragment().into();
        view_fn.render().mount(&fragment_node, attrs.clone());

        if let Some(parent) = end_node.parent_node() {
            let _ = parent.insert_before(&fragment_node, Some(&end_node));
        }
    });
}
