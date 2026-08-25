/// Node categories shared by browser and SSR implementations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Document,
    Element,
    Text,
    Comment,
    Fragment,
}

impl NodeKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Element => "element",
            Self::Text => "text",
            Self::Comment => "comment",
            Self::Fragment => "fragment",
        }
    }
}

/// HTML/XML namespace metadata used by element creation and serialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Namespace {
    Html,
    Svg,
    MathMl,
    Custom(String),
}

impl Namespace {
    pub fn uri(&self) -> Option<&str> {
        match self {
            Self::Html => None,
            Self::Svg => Some("http://www.w3.org/2000/svg"),
            Self::MathMl => Some("http://www.w3.org/1998/Math/MathML"),
            Self::Custom(uri) => Some(uri),
        }
    }
}

/// Backend-independent element metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementSpec {
    name: String,
    namespace: Namespace,
    void: bool,
}

impl ElementSpec {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let void = is_html_void_name(&name);
        Self {
            name,
            namespace: Namespace::Html,
            void,
        }
    }

    pub fn namespaced(name: impl Into<String>, namespace: Namespace, void: bool) -> Self {
        Self {
            name: name.into(),
            namespace,
            void,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn namespace(&self) -> &Namespace {
        &self.namespace
    }

    pub fn is_void(&self) -> bool {
        self.void
    }
}

fn is_html_void_name(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}
