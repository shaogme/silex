use crate::SilexError;
use crate::dom::attribute::AttributeValue;
use crate::dom::tags::Tag;
use crate::dom::view::View;
use crate::reactivity::on_cleanup;

use super::tags;
use std::marker::PhantomData;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::Element as WebElem;

/// 基础 DOM 元素包装器
#[derive(Clone)]
pub struct Element {
    pub dom_element: WebElem,
}

pub fn mount_to_body<V: View>(view: V) {
    let document = crate::dom::document();
    let body = document.body().expect("No body element");
    view.mount(&body);
}

impl Element {
    pub fn new(tag: &str) -> Self {
        let document = crate::dom::document();
        let dom_element = document
            .create_element(tag)
            .expect("Failed to create element");
        Self { dom_element }
    }

    pub fn new_svg(tag: &str) -> Self {
        let document = crate::dom::document();
        let dom_element = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
            .expect("Failed to create SVG element");
        Self { dom_element }
    }

    // --- 统一的属性 API ---

    pub fn attr(self, name: &str, value: impl AttributeValue) -> Self {
        value.apply(&self.dom_element, name);
        self
    }

    pub fn id(self, value: impl AttributeValue) -> Self {
        self.attr("id", value)
    }

    pub fn class(self, value: impl AttributeValue) -> Self {
        self.attr("class", value)
    }

    pub fn style(self, value: impl AttributeValue) -> Self {
        self.attr("style", value)
    }

    // --- 事件 API ---

    pub fn on_click<F>(self, callback: F) -> Self
    where
        F: Fn(web_sys::MouseEvent) + 'static,
    {
        let closure = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            callback(e);
        }) as Box<dyn FnMut(_)>);

        let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
        if let Err(e) = self
            .dom_element
            .add_event_listener_with_callback("click", js_value)
            .map_err(SilexError::from)
        {
            crate::error::handle_error(e);
            return self;
        }

        let target = self.dom_element.clone();
        let js_fn = js_value.clone();

        // 注册清理回调
        on_cleanup(move || {
            let _ = target.remove_event_listener_with_callback("click", &js_fn);
            drop(closure);
        });

        self
    }

    pub fn on_input<F>(self, mut callback: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        let closure = Closure::wrap(Box::new(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                let input = target.unchecked_into::<web_sys::HtmlInputElement>();
                callback(input.value());
            } else {
                let err = SilexError::Dom("Input event has no target".into());
                crate::error::handle_error(err);
            }
        }) as Box<dyn FnMut(_)>);

        let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
        if let Err(e) = self
            .dom_element
            .add_event_listener_with_callback("input", js_value)
            .map_err(SilexError::from)
        {
            crate::error::handle_error(e);
            return self;
        }

        let target = self.dom_element.clone();
        let js_fn = js_value.clone();

        // 注册清理回调
        on_cleanup(move || {
            let _ = target.remove_event_listener_with_callback("input", &js_fn);
            drop(closure);
        });

        self
    }

    // --- 统一的子节点/文本 API ---

    pub fn child<V: View>(self, view: V) -> Self {
        view.mount(&self.dom_element);
        self
    }

    pub fn text<V: View>(self, content: V) -> Self {
        self.child(content)
    }
}

impl std::ops::Deref for Element {
    type Target = WebElem;

    fn deref(&self) -> &Self::Target {
        &self.dom_element
    }
}

/// Type-safe wrapper for DOM elements
#[derive(Clone)]
pub struct TypedElement<T> {
    pub element: Element,
    _marker: PhantomData<T>,
}

impl<T> TypedElement<T> {
    pub(crate) fn new(tag: &str) -> Self {
        let document = crate::dom::document();
        let dom_element = document
            .create_element(tag)
            .expect("Failed to create element");
        Self {
            element: Element { dom_element },
            _marker: PhantomData,
        }
    }

    pub(crate) fn new_svg(tag: &str) -> Self {
        let document = crate::dom::document();
        let dom_element = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
            .expect("Failed to create SVG element");
        Self {
            element: Element { dom_element },
            _marker: PhantomData,
        }
    }

    pub fn into_untyped(self) -> Element {
        self.element
    }

    // --- Unified Attribute API ---

    pub fn attr(self, name: &str, value: impl AttributeValue) -> Self {
        value.apply(&self.element, name);
        self
    }

    pub fn id(self, value: impl AttributeValue) -> Self {
        self.attr("id", value)
    }

    pub fn class(self, value: impl AttributeValue) -> Self {
        self.attr("class", value)
    }

    pub fn style(self, value: impl AttributeValue) -> Self {
        self.attr("style", value)
    }

    // --- Event API (Duplicated from Element) ---

    pub fn on_click<F>(self, callback: F) -> Self
    where
        F: Fn(web_sys::MouseEvent) + 'static,
    {
        let closure = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            callback(e);
        }) as Box<dyn FnMut(_)>);

        let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
        if let Err(e) = self
            .element
            .add_event_listener_with_callback("click", js_value)
            .map_err(SilexError::from)
        {
            crate::error::handle_error(e);
            return self;
        }

        let target = self.element.clone();
        let js_fn = js_value.clone();

        on_cleanup(move || {
            let _ = target.remove_event_listener_with_callback("click", &js_fn);
            drop(closure);
        });

        self
    }

    pub fn on_input<F>(self, mut callback: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        let closure = Closure::wrap(Box::new(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                let input = target.unchecked_into::<web_sys::HtmlInputElement>();
                callback(input.value());
            } else {
                let err = SilexError::Dom("Input event has no target".into());
                crate::error::handle_error(err);
            }
        }) as Box<dyn FnMut(_)>);

        let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
        if let Err(e) = self
            .element
            .add_event_listener_with_callback("input", js_value)
            .map_err(SilexError::from)
        {
            crate::error::handle_error(e);
            return self;
        }

        let target = self.element.clone();
        let js_fn = js_value.clone();

        on_cleanup(move || {
            let _ = target.remove_event_listener_with_callback("input", &js_fn);
            drop(closure);
        });

        self
    }

    // --- Children API ---

    pub fn child<V: View>(self, view: V) -> Self {
        view.mount(&self.element);
        self
    }

    pub fn text<V: View>(self, content: V) -> Self {
        self.child(content)
    }
}

impl<T: Tag> Into<Element> for TypedElement<T> {
    fn into(self) -> Element {
        self.into_untyped()
    }
}

impl<T: Tag> std::ops::Deref for TypedElement<T> {
    type Target = Element;
    fn deref(&self) -> &Self::Target {
        &self.element
    }
}

pub mod tag {
    use super::tags::*;
    use super::*;

    // --- HTML Tags ---
    pub fn div() -> TypedElement<Div> {
        TypedElement::new("div")
    }
    pub fn span() -> TypedElement<Span> {
        TypedElement::new("span")
    }
    pub fn h1() -> TypedElement<H1> {
        TypedElement::new("h1")
    }
    pub fn h2() -> TypedElement<H2> {
        TypedElement::new("h2")
    }
    pub fn h3() -> TypedElement<H3> {
        TypedElement::new("h3")
    }
    pub fn h4() -> TypedElement<H4> {
        TypedElement::new("h4")
    }
    pub fn h5() -> TypedElement<H5> {
        TypedElement::new("h5")
    }
    pub fn h6() -> TypedElement<H6> {
        TypedElement::new("h6")
    }
    pub fn p() -> TypedElement<P> {
        TypedElement::new("p")
    }
    pub fn a() -> TypedElement<A> {
        TypedElement::new("a")
    }
    pub fn button() -> TypedElement<Button> {
        TypedElement::new("button")
    }
    pub fn img() -> TypedElement<Img> {
        TypedElement::new("img")
    }
    pub fn input() -> TypedElement<Input> {
        TypedElement::new("input")
    }
    pub fn ul() -> TypedElement<Ul> {
        TypedElement::new("ul")
    }
    pub fn ol() -> TypedElement<Ol> {
        TypedElement::new("ol")
    }
    pub fn li() -> TypedElement<Li> {
        TypedElement::new("li")
    }
    pub fn nav() -> TypedElement<Nav> {
        TypedElement::new("nav")
    }
    pub fn main() -> TypedElement<Main> {
        TypedElement::new("main")
    }
    pub fn footer() -> TypedElement<Footer> {
        TypedElement::new("footer")
    }
    pub fn aside() -> TypedElement<Aside> {
        TypedElement::new("aside")
    }
    pub fn br() -> TypedElement<Br> {
        TypedElement::new("br")
    }
    pub fn hr() -> TypedElement<Hr> {
        TypedElement::new("hr")
    }
    pub fn article() -> TypedElement<Article> {
        TypedElement::new("article")
    }
    pub fn header() -> TypedElement<Header> {
        TypedElement::new("header")
    }
    pub fn time() -> TypedElement<Time> {
        TypedElement::new("time")
    }
    pub fn figure() -> TypedElement<Figure> {
        TypedElement::new("figure")
    }
    pub fn figcaption() -> TypedElement<Figcaption> {
        TypedElement::new("figcaption")
    }
    pub fn blockquote() -> TypedElement<Blockquote> {
        TypedElement::new("blockquote")
    }
    pub fn pre() -> TypedElement<Pre> {
        TypedElement::new("pre")
    }
    pub fn code() -> TypedElement<Code> {
        TypedElement::new("code")
    }
    pub fn em() -> TypedElement<Em> {
        TypedElement::new("em")
    }
    pub fn strong() -> TypedElement<Strong> {
        TypedElement::new("strong")
    }
    pub fn s() -> TypedElement<S> {
        TypedElement::new("s")
    }
    pub fn table() -> TypedElement<Table> {
        TypedElement::new("table")
    }
    pub fn thead() -> TypedElement<Thead> {
        TypedElement::new("thead")
    }
    pub fn tbody() -> TypedElement<Tbody> {
        TypedElement::new("tbody")
    }
    pub fn tr() -> TypedElement<Tr> {
        TypedElement::new("tr")
    }
    pub fn td() -> TypedElement<Td> {
        TypedElement::new("td")
    }
    pub fn label() -> TypedElement<Label> {
        TypedElement::new("label")
    }
    pub fn section() -> TypedElement<Section> {
        TypedElement::new("section")
    }

    // --- SVG Tags ---
    pub fn svg() -> TypedElement<Svg> {
        TypedElement::new_svg("svg")
    }
    pub fn path() -> TypedElement<Path> {
        TypedElement::new_svg("path")
    }
    pub fn defs() -> TypedElement<Defs> {
        TypedElement::new_svg("defs")
    }
    pub fn filter() -> TypedElement<Filter> {
        TypedElement::new_svg("filter")
    }
    pub fn fe_turbulence() -> TypedElement<FeTurbulence> {
        TypedElement::new_svg("feTurbulence")
    }
    pub fn fe_component_transfer() -> TypedElement<FeComponentTransfer> {
        TypedElement::new_svg("feComponentTransfer")
    }
    pub fn fe_func_r() -> TypedElement<FeFuncR> {
        TypedElement::new_svg("feFuncR")
    }
    pub fn fe_func_g() -> TypedElement<FeFuncG> {
        TypedElement::new_svg("feFuncG")
    }
    pub fn fe_func_b() -> TypedElement<FeFuncB> {
        TypedElement::new_svg("feFuncB")
    }
    pub fn fe_gaussian_blur() -> TypedElement<FeGaussianBlur> {
        TypedElement::new_svg("feGaussianBlur")
    }
    pub fn fe_specular_lighting() -> TypedElement<FeSpecularLighting> {
        TypedElement::new_svg("feSpecularLighting")
    }
    pub fn fe_point_light() -> TypedElement<FePointLight> {
        TypedElement::new_svg("fePointLight")
    }
    pub fn fe_composite() -> TypedElement<FeComposite> {
        TypedElement::new_svg("feComposite")
    }
    pub fn fe_displacement_map() -> TypedElement<FeDisplacementMap> {
        TypedElement::new_svg("feDisplacementMap")
    }
    pub fn g() -> TypedElement<G> {
        TypedElement::new_svg("g")
    }
    pub fn rect() -> TypedElement<Rect> {
        TypedElement::new_svg("rect")
    }
    pub fn circle() -> TypedElement<Circle> {
        TypedElement::new_svg("circle")
    }
    pub fn line() -> TypedElement<Line> {
        TypedElement::new_svg("line")
    }
    pub fn polyline() -> TypedElement<Polyline> {
        TypedElement::new_svg("polyline")
    }
    pub fn polygon() -> TypedElement<Polygon> {
        TypedElement::new_svg("polygon")
    }
}
