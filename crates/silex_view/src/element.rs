use crate::any::AnyView;
use crate::attribute::{ApplyTarget, AttrOp, AttributeBuilder, IntoStorable};
use crate::context::{MountContext, MountTarget};
use crate::contract::{MountInstance, View};
use crate::event::{EventDescriptor, EventHandler, bind_event};
use silex_core::{CloseError, SilexError, SilexErrorKind, SilexResult};
use silex_dom::{ElementSpec, Namespace};
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// HTML/SVG tag 的命名空间元数据。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagNamespace {
    Html,
    Svg,
}

/// 由 HTML/SVG codegen 生成的稳定 tag 元数据。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagMetadata {
    pub name: &'static str,
    pub namespace: TagNamespace,
    pub is_void: bool,
}

impl TagMetadata {
    pub const fn new(name: &'static str, namespace: TagNamespace, is_void: bool) -> Self {
        Self {
            name,
            namespace,
            is_void,
        }
    }
}

/// Tag marker。它只携带 View metadata 和 capability marker，不携带 browser 类型。
pub trait Tag {
    const METADATA: TagMetadata;
}

pub trait FormTag: Tag {}
pub trait LabelTag: Tag {}
pub trait AnchorTag: Tag {}
pub trait MediaTag: Tag {}
pub trait TextTag: Tag {}
pub trait OpenTag: Tag {}
pub trait TableCellTag: Tag {}
pub trait TableHeaderTag: Tag {}
pub trait SvgTag: Tag {}

/// 定义不携带 browser concrete type 的 View tag marker 和 builder。
#[macro_export]
macro_rules! define_tag {
    ($struct_name:ident, $tag_name:literal, html, $fn_name:ident, void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::Tag for $struct_name {
            const METADATA: $crate::TagMetadata =
                $crate::TagMetadata::new($tag_name, $crate::TagNamespace::Html, true);
        }
        $(impl $crate::element::$traits for $struct_name {})*
        pub fn $fn_name<'scope>() -> $crate::TypedElement<'scope, $struct_name> {
            $crate::TypedElement::from_tag()
        }
    };
    ($struct_name:ident, $tag_name:literal, html, $fn_name:ident, non_void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::Tag for $struct_name {
            const METADATA: $crate::TagMetadata =
                $crate::TagMetadata::new($tag_name, $crate::TagNamespace::Html, false);
        }
        $(impl $crate::element::$traits for $struct_name {})*
        pub fn $fn_name<'scope, V>(child: V) -> $crate::TypedElement<'scope, $struct_name>
        where
            V: $crate::View<'scope> + 'scope,
        {
            $crate::TypedElement::with_child_from_tag(child)
        }
    };
    ($struct_name:ident, $tag_name:literal, svg, $fn_name:ident, void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::Tag for $struct_name {
            const METADATA: $crate::TagMetadata =
                $crate::TagMetadata::new($tag_name, $crate::TagNamespace::Svg, true);
        }
        $(impl $crate::element::$traits for $struct_name {})*
        pub fn $fn_name<'scope>() -> $crate::TypedElement<'scope, $struct_name> {
            $crate::TypedElement::from_tag()
        }
    };
    ($struct_name:ident, $tag_name:literal, svg, $fn_name:ident, non_void, [$($traits:ident),*]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $struct_name;
        impl $crate::Tag for $struct_name {
            const METADATA: $crate::TagMetadata =
                $crate::TagMetadata::new($tag_name, $crate::TagNamespace::Svg, false);
        }
        $(impl $crate::element::$traits for $struct_name {})*
        pub fn $fn_name<'scope, V>(child: V) -> $crate::TypedElement<'scope, $struct_name>
        where
            V: $crate::View<'scope> + 'scope,
        {
            $crate::TypedElement::with_child_from_tag(child)
        }
    };
}

pub fn text<'scope, V: View<'scope>>(content: V) -> V {
    content
}

pub struct Element<'scope> {
    tag_name: String,
    namespace: Option<Namespace>,
    is_void: bool,
    pub(crate) pending_attrs: Vec<AttrOp<'scope>>,
    pub(crate) children: Vec<AnyView<'scope>>,
}

impl<'scope> Clone for Element<'scope> {
    fn clone(&self) -> Self {
        Self {
            tag_name: self.tag_name.clone(),
            namespace: self.namespace.clone(),
            is_void: self.is_void,
            pending_attrs: self.pending_attrs.clone(),
            children: self.children.clone(),
        }
    }
}

impl PartialEq for Element<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.tag_name == other.tag_name
            && self.namespace == other.namespace
            && self.is_void == other.is_void
            && self.pending_attrs == other.pending_attrs
            && self.children == other.children
    }
}

impl<'scope> Element<'scope> {
    pub fn new(tag: &str) -> Self {
        Self {
            tag_name: tag.to_string(),
            namespace: None,
            is_void: false,
            pending_attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn new_svg(tag: &str) -> Self {
        Self {
            tag_name: tag.to_string(),
            namespace: Some(Namespace::Svg),
            is_void: false,
            pending_attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn with_child<V>(tag: &str, child: V) -> Self
    where
        V: View<'scope> + 'scope,
    {
        let mut element = Self::new(tag);
        element.children.push(child.into_any());
        element
    }

    fn spec(&self) -> ElementSpec {
        self.namespace.clone().map_or_else(
            || {
                if self.is_void {
                    ElementSpec::namespaced(self.tag_name.clone(), Namespace::Html, true)
                } else {
                    ElementSpec::new(self.tag_name.clone())
                }
            },
            |namespace| ElementSpec::namespaced(self.tag_name.clone(), namespace, self.is_void),
        )
    }

    fn mount_inner(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        let dom_element = context
            .dom()
            .create_element(self.spec())
            .map_err(crate::error::dom_error)?;
        let parent_owner = context.owner();
        let provisional_owner = parent_owner.child();
        let token = provisional_owner.clone();
        let cleanup_dom = context.dom().clone();
        let cleanup_handler = context.error_handler();
        let mut appended = false;
        let result = (|| -> SilexResult<MountInstance<'scope>> {
            let child_context = context.with_parts(
                MountTarget::append(context.dom().clone(), dom_element.node().clone()),
                context.ancestry().push(&dom_element),
                token.clone(),
                context.transaction().clone(),
            );
            context.target().append_node(dom_element.node())?;
            appended = true;
            for attr in crate::attribute::consolidate_attributes(self.pending_attrs.clone()) {
                attr.apply(&dom_element, &child_context)?;
            }
            for child in &self.children {
                let _ = child_context.mount(child)?;
            }
            let owner_for_cleanup = provisional_owner.clone();
            let element_for_cleanup = dom_element.node().clone();
            parent_owner.on_cleanup(
                Box::new(move || {
                    owner_for_cleanup
                        .close()
                        .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))?;
                    if cleanup_dom
                        .parent(&element_for_cleanup)
                        .map_err(crate::error::dom_error)?
                        .is_some()
                    {
                        cleanup_dom
                            .remove(&element_for_cleanup)
                            .map_err(crate::error::dom_error)?;
                    }
                    Ok(())
                }),
                cleanup_handler,
            )?;
            Ok(MountInstance::from_nodes(vec![dom_element.node().clone()]))
        })();
        if let Err(error) = &result {
            rollback_mount(context, &provisional_owner, dom_element.node(), appended);
            return Err(error.clone());
        }
        result
    }
}

fn rollback_mount<'scope>(
    context: &MountContext<'scope>,
    owner: &crate::owner::MountOwnerToken<'scope>,
    element: &silex_dom::DomNode,
    appended: bool,
) {
    let close_result = catch_unwind(AssertUnwindSafe(|| owner.close()));
    if appended && context.dom().parent(element).ok().flatten().is_some() {
        let _ = context.dom().remove(element);
    }
    match close_result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => owner.report_close_error(error),
        Err(panic) => owner.report_close_error(CloseError::from_panic(panic)),
    }
}

impl<'scope> AttributeBuilder<'scope> for Element<'scope> {
    fn build_attribute<V>(mut self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.pending_attrs
            .push(AttrOp::build(value.into_storable(), target));
        self
    }

    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.pending_attrs
            .push(AttrOp::new_scoped(move |element, context| {
                bind_event(
                    context,
                    element,
                    event,
                    callback.clone(),
                    context.error_handler(),
                )
            }));
        self
    }
}

impl<'scope> View<'scope> for Element<'scope> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        self.mount_inner(context)
    }
}

pub struct TypedElement<'scope, T: Tag> {
    tag_name: String,
    namespace: Option<Namespace>,
    is_void: bool,
    pub(crate) pending_attrs: Vec<AttrOp<'scope>>,
    pub(crate) children: Vec<AnyView<'scope>>,
    marker: PhantomData<T>,
}

impl<'scope, T: Tag> Clone for TypedElement<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            tag_name: self.tag_name.clone(),
            namespace: self.namespace.clone(),
            is_void: self.is_void,
            pending_attrs: self.pending_attrs.clone(),
            children: self.children.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: Tag> PartialEq for TypedElement<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.tag_name == other.tag_name
            && self.namespace == other.namespace
            && self.is_void == other.is_void
            && self.pending_attrs == other.pending_attrs
            && self.children == other.children
    }
}

impl<'scope, T: Tag> TypedElement<'scope, T> {
    pub fn new(tag: &str) -> Self {
        Self {
            tag_name: tag.to_string(),
            namespace: None,
            is_void: false,
            pending_attrs: Vec::new(),
            children: Vec::new(),
            marker: PhantomData,
        }
    }
    pub fn new_svg(tag: &str) -> Self {
        Self {
            tag_name: tag.to_string(),
            namespace: Some(Namespace::Svg),
            is_void: false,
            pending_attrs: Vec::new(),
            children: Vec::new(),
            marker: PhantomData,
        }
    }
    pub fn with_child<V>(tag: &str, child: V) -> Self
    where
        V: View<'scope> + 'scope,
    {
        let mut element = Self::new(tag);
        element.children.push(child.into_any());
        element
    }

    /// 从 codegen 提供的 tag metadata 创建 typed element。
    pub fn from_tag() -> Self {
        let metadata = T::METADATA;
        let (tag_name, namespace) = match metadata.namespace {
            TagNamespace::Html => (metadata.name.to_string(), None),
            TagNamespace::Svg => (metadata.name.to_string(), Some(Namespace::Svg)),
        };
        Self {
            tag_name,
            namespace,
            is_void: metadata.is_void,
            pending_attrs: Vec::new(),
            children: Vec::new(),
            marker: PhantomData,
        }
    }

    /// 从 codegen 提供的 tag metadata 创建带一个 child 的 typed element。
    pub fn with_child_from_tag<V>(child: V) -> Self
    where
        V: View<'scope> + 'scope,
    {
        let mut element = Self::from_tag();
        element.children.push(child.into_any());
        element
    }
    #[doc(hidden)]
    pub fn with_child_using<V>(tag: &str, child: V, constructor: fn(&str) -> Self) -> Self
    where
        V: View<'scope> + 'scope,
    {
        let mut element = constructor(tag);
        element.children.push(child.into_any());
        element
    }
    pub fn into_untyped(self) -> Element<'scope> {
        Element {
            tag_name: self.tag_name,
            namespace: self.namespace,
            is_void: self.is_void,
            pending_attrs: self.pending_attrs,
            children: self.children,
        }
    }
}

impl<'scope, T: Tag> AttributeBuilder<'scope> for TypedElement<'scope, T> {
    fn build_attribute<V>(mut self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.pending_attrs
            .push(AttrOp::build(value.into_storable(), target));
        self
    }
    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.pending_attrs
            .push(AttrOp::new_scoped(move |element, context| {
                bind_event(
                    context,
                    element,
                    event,
                    callback.clone(),
                    context.error_handler(),
                )
            }));
        self
    }
}

impl<'scope, T: Tag> View<'scope> for TypedElement<'scope, T> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        let view = self.clone().into_untyped();
        context.mount(&view)
    }
}

impl<'scope, T: Tag> From<TypedElement<'scope, T>> for Element<'scope> {
    fn from(value: TypedElement<'scope, T>) -> Self {
        value.into_untyped()
    }
}
