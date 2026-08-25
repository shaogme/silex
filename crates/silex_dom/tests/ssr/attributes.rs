use silex_dom::{
    adapters::ssr::{SerializeOptions, SsrDom},
    model::{
        attribute::{
            AttributeRequest, AttributeTarget, AttributeValue, PropertyRequest, PropertyValue,
        },
        node::ElementSpec,
    },
};

#[test]
fn ssr_tree_serializes_deterministically_and_escapes_values() {
    let dom = SsrDom::new();
    let context = dom.context();
    let document = context.document().expect("document should exist");
    let root = context
        .create_element(ElementSpec::new("div"))
        .expect("element should exist");
    let text = context.create_text("<&>").expect("text should exist");
    context
        .set_attribute(AttributeRequest::new(
            &root,
            AttributeTarget::named("data-value"),
            AttributeValue::text("\"<&"),
        ))
        .expect("attribute should be written");
    context
        .set_attribute(AttributeRequest::new(
            &root,
            AttributeTarget::Style,
            AttributeValue::text("color:red<&"),
        ))
        .expect("style should be written");
    context
        .set_property(PropertyRequest::new(
            &root,
            "value",
            PropertyValue::string("not an attribute"),
        ))
        .expect("property should be written");
    context
        .append(root.node(), &text)
        .expect("append should work");
    context
        .append(document.node(), root.node())
        .expect("root should attach");
    assert_eq!(
        dom.serialize(SerializeOptions::default())
            .expect("serialization should work"),
        "<div data-value=\"&quot;&lt;&amp;\" style=\"color:red&lt;&amp;\">&lt;&amp;&gt;</div>"
    );
}

#[test]
fn ssr_style_property_updates_inline_style_and_has_no_body_target() {
    let dom = SsrDom::new();
    let context = dom.context();
    let root = context
        .create_element(ElementSpec::new("div"))
        .expect("element should exist");
    context
        .set_attribute(AttributeRequest::new(
            &root,
            AttributeTarget::Style,
            AttributeValue::text("color:red;"),
        ))
        .expect("style should be written");
    context
        .set_style_property(&root, "--dynamic", Some("blue"))
        .expect("style property should be written");
    assert_eq!(
        dom.serialize_node(&root.node().clone(), SerializeOptions::default())
            .expect("element should serialize"),
        "<div style=\"--dynamic:blue;color:red;\"></div>"
    );
    context
        .set_style_property(&root, "--dynamic", None)
        .expect("style property should be removed");
    assert_eq!(
        dom.serialize_node(&root.node().clone(), SerializeOptions::default())
            .expect("element should serialize"),
        "<div style=\"color:red;\"></div>"
    );
    assert!(
        context
            .document_body()
            .expect("SSR body capability should be queryable")
            .is_none()
    );
}
