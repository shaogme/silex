use super::erased::AnyView;
use super::{
    tag::{Tag, TagNamespace},
    untyped::Element,
};
use crate::events::{EventDescriptor, EventHandler, bind_event};
use crate::kernel::attributes::{ApplyTarget, AttrOp, AttributeBuilder, IntoStorable};
use crate::kernel::{MountContext, MountInstance, View};
use silex_core::SilexResult;
use silex_dom::model::Namespace;
use std::marker::PhantomData;
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
