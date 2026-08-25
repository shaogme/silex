use std::error::Error;

use silex_dom::{
    adapters::ssr::{SerializeOptions, SsrDom},
    model::{
        attribute::{AttributeRequest, AttributeTarget, AttributeValue},
        node::ElementSpec,
    },
};

pub fn run() -> Result<(), Box<dyn Error>> {
    let dom = SsrDom::new();
    let context = dom.context();
    let document = context.document()?;
    let main = context.create_element(ElementSpec::new("main"))?;
    let text = context.create_text("hello")?;

    context.set_attribute(AttributeRequest::new(
        &main,
        AttributeTarget::named("data-example"),
        AttributeValue::text("silex_dom"),
    ))?;
    context.append(main.node(), &text)?;
    context.append(document.node(), main.node())?;

    let html = dom.serialize(SerializeOptions::default())?;
    if html != "<main data-example=\"silex_dom\">hello</main>" {
        return Err(format!("unexpected SSR output: {html}").into());
    }
    Ok(())
}
