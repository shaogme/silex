use silex_dom::attribute::AttrOp;

fn make_attribute<'scope>(value: &'scope str) -> AttrOp<'static> {
    AttrOp::new_listener(move |_| {
        let _ = value.len();
        Ok(())
    })
}

fn main() {
    let value = String::from("scoped-attribute");
    let _attribute = make_attribute(&value);
}
