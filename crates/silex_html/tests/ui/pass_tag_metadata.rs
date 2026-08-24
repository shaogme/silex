use silex_html::{A, Br, SvgA, a, br, svg_a};
use silex_view::{Tag, TagNamespace, TypedElement};

fn main() {
    assert_eq!(A::METADATA.name, "a");
    assert_eq!(A::METADATA.namespace, TagNamespace::Html);
    assert!(!A::METADATA.is_void);

    assert_eq!(Br::METADATA.name, "br");
    assert!(Br::METADATA.is_void);

    assert_eq!(SvgA::METADATA.namespace, TagNamespace::Svg);
    assert!(!SvgA::METADATA.is_void);

    let _: TypedElement<'_, A> = a("link");
    let _: TypedElement<'_, Br> = br();
    let _: TypedElement<'_, SvgA> = svg_a("icon");
}
