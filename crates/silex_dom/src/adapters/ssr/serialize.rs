use crate::{
    diagnostics::error::{DomError, DomResult},
    model::node::{Namespace, NodeKind},
};

use super::state::{NodeId, SsrState};

/// Serialization policy. Raw HTML is deliberately not representable here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SerializeOptions {
    pub include_comments: bool,
}

impl Default for SerializeOptions {
    fn default() -> Self {
        Self {
            include_comments: true,
        }
    }
}

pub(super) fn serialize_node(
    state: &SsrState,
    id: NodeId,
    options: &SerializeOptions,
    parent_namespace: Option<&Namespace>,
    output: &mut String,
) -> DomResult<()> {
    let node = state.nodes.get(&id).ok_or(DomError::InvalidHandle {
        backend: 0,
        kind: "node",
    })?;
    match node.kind {
        NodeKind::Document | NodeKind::Fragment => {
            for child in &node.children {
                serialize_node(state, *child, options, parent_namespace, output)?;
            }
        }
        NodeKind::Text => escape_text(node.text.as_deref().unwrap_or_default(), output),
        NodeKind::Comment => {
            if options.include_comments {
                output.push_str("<!--");
                escape_comment(node.text.as_deref().unwrap_or_default(), output);
                output.push_str("-->");
            }
        }
        NodeKind::Element => {
            let name = node.name.as_deref().unwrap_or_default();
            let namespace = node.namespace.as_ref().ok_or(DomError::Backend {
                operation: "serialize",
                message: String::from("element namespace is missing"),
            })?;
            output.push('<');
            escape_name(name, output);
            if parent_namespace != Some(namespace)
                && let Some(uri) = namespace.uri()
            {
                output.push_str(" xmlns=\"");
                escape_attribute(uri, output);
                output.push('"');
            }
            for (key, value) in &node.attributes {
                output.push(' ');
                escape_name(key, output);
                output.push_str("=\"");
                escape_attribute(value, output);
                output.push('"');
            }
            output.push('>');
            if !node.void {
                for child in &node.children {
                    serialize_node(state, *child, options, Some(namespace), output)?;
                }
                output.push_str("</");
                escape_name(name, output);
                output.push('>');
            }
        }
    }
    Ok(())
}

fn escape_text(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
}

fn escape_attribute(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(character),
        }
    }
}

fn escape_name(value: &str, output: &mut String) {
    escape_attribute(value, output);
}

fn escape_comment(value: &str, output: &mut String) {
    output.push_str(&value.replace("--", "- -"));
    if value.ends_with('-') {
        output.push(' ');
    }
}
