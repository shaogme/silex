use syn::{Ident, Result};

#[derive(Clone, Debug)]
pub(crate) struct HtmlTagSpec {
    pub(crate) marker: Ident,
    pub(crate) is_void: bool,
}

impl HtmlTagSpec {
    pub(crate) fn from_tag(tag: &Ident) -> Result<Option<Self>> {
        let tag_name = tag.to_string();
        if !tag_name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
        {
            return Ok(None);
        }

        if !HTML_TAG_NAMES.contains(&tag_name.as_str()) {
            return Err(syn::Error::new(
                tag.span(),
                format!("unknown HTML tag `{tag_name}`"),
            ));
        }

        let marker_name = match tag_name.as_str() {
            "a" => "A",
            "data" => "DataTag",
            "option" => "OptionTag",
            "param" => "Param",
            "time" => "Time",
            _ => {
                let mut characters = tag_name.chars();
                let first = characters
                    .next()
                    .expect("HTML tag names are not empty")
                    .to_uppercase()
                    .collect::<String>();
                return Ok(Some(Self {
                    marker: Ident::new(
                        &format!("{first}{}", characters.collect::<String>()),
                        tag.span(),
                    ),
                    is_void: HTML_VOID_TAG_NAMES.contains(&tag_name.as_str()),
                }));
            }
        };

        Ok(Some(Self {
            marker: Ident::new(marker_name, tag.span()),
            is_void: HTML_VOID_TAG_NAMES.contains(&tag_name.as_str()),
        }))
    }
}

const HTML_TAG_NAMES: &[&str] = &[
    "a",
    "abbr",
    "acronym",
    "address",
    "area",
    "article",
    "aside",
    "audio",
    "b",
    "base",
    "bdi",
    "bdo",
    "big",
    "blockquote",
    "body",
    "br",
    "button",
    "canvas",
    "caption",
    "center",
    "cite",
    "code",
    "col",
    "colgroup",
    "data",
    "datalist",
    "dd",
    "del",
    "details",
    "dfn",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "fencedframe",
    "fieldset",
    "figcaption",
    "figure",
    "font",
    "footer",
    "form",
    "frame",
    "frameset",
    "geolocation",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "i",
    "iframe",
    "img",
    "input",
    "ins",
    "kbd",
    "label",
    "legend",
    "li",
    "link",
    "main",
    "map",
    "mark",
    "marquee",
    "menu",
    "meta",
    "meter",
    "nav",
    "nobr",
    "noembed",
    "noframes",
    "noscript",
    "object",
    "ol",
    "optgroup",
    "option",
    "output",
    "p",
    "param",
    "picture",
    "plaintext",
    "pre",
    "progress",
    "q",
    "rb",
    "rp",
    "rt",
    "rtc",
    "ruby",
    "s",
    "samp",
    "script",
    "search",
    "section",
    "select",
    "selectedcontent",
    "slot",
    "small",
    "source",
    "span",
    "strike",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "time",
    "title",
    "tr",
    "track",
    "tt",
    "u",
    "ul",
    "var",
    "video",
    "wbr",
    "xmp",
];

const HTML_VOID_TAG_NAMES: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

#[cfg(test)]
mod tests {
    use super::HtmlTagSpec;

    fn spec(tag: &str) -> HtmlTagSpec {
        HtmlTagSpec::from_tag(&syn::parse_str(tag).expect("tag should parse"))
            .expect("tag should be valid")
            .expect("HTML tag should have a spec")
    }

    #[test]
    fn maps_representative_tags_to_markers_and_void_status() {
        let cases = [
            ("button", "Button", false),
            ("textarea", "Textarea", false),
            ("a", "A", false),
            ("img", "Img", true),
            ("label", "Label", false),
            ("dialog", "Dialog", false),
            ("td", "Td", false),
            ("th", "Th", false),
        ];

        for (tag, marker, is_void) in cases {
            let actual = spec(tag);
            assert_eq!(actual.marker.to_string(), marker);
            assert_eq!(actual.is_void, is_void);
        }
    }

    #[test]
    fn keeps_custom_component_names_out_of_html_specs() {
        let tag = syn::parse_str("CustomTag").expect("tag should parse");
        assert!(
            HtmlTagSpec::from_tag(&tag)
                .expect("custom component should be accepted")
                .is_none()
        );
    }

    #[test]
    fn rejects_unknown_lowercase_tags() {
        let tag = syn::parse_str("unknown_tag").expect("tag should parse");
        let error = HtmlTagSpec::from_tag(&tag).expect_err("unknown tag should fail");
        assert!(error.to_string().contains("unknown HTML tag"));
    }
}
