use silex_dom::attribute::PendingAttribute;

fn make_attribute<'scope>(value: &'scope str) -> PendingAttribute<'static, 'static> {
    PendingAttribute::new_listener(move |_| {
        let _ = value.len();
    })
}

fn main() {
    let value = String::from("scoped-attribute");
    let _attribute = make_attribute(&value);
}
