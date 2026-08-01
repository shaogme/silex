use crate::{I18nError, Locale, PluralCategory};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Segment {
    Literal(String),
    Argument(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluralForms {
    forms: BTreeMap<PluralCategory, Vec<Segment>>,
}

impl PluralForms {
    pub fn from_templates<I, K, V>(forms: I) -> Result<Self, I18nError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self::from_template_pairs("<plural>", forms)
    }

    pub fn get(&self, category: PluralCategory) -> Option<&[Segment]> {
        self.forms.get(&category).map(Vec::as_slice)
    }

    pub fn contains(&self, category: PluralCategory) -> bool {
        self.forms.contains_key(&category)
    }

    pub fn iter(&self) -> impl Iterator<Item = (PluralCategory, &[Segment])> {
        self.forms
            .iter()
            .map(|(category, segments)| (*category, segments.as_slice()))
    }

    fn from_template_pairs<I, K, V>(key: &str, forms: I) -> Result<Self, I18nError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut parsed = BTreeMap::new();
        for (category, template) in forms {
            let category = category.into();
            let category = PluralCategory::from_name(&category).ok_or_else(|| {
                I18nError::InvalidCatalog(format!(
                    "invalid plural category '{category}' for '{key}'"
                ))
            })?;
            if parsed.contains_key(&category) {
                return Err(I18nError::InvalidCatalog(format!(
                    "duplicate plural category '{}' for '{key}'",
                    category.as_str()
                )));
            }
            let template = template.into();
            let segments = tokenize_template(key, &template)?;
            parsed.insert(category, segments);
        }

        if !parsed.contains_key(&PluralCategory::Other) {
            return Err(I18nError::MissingOther {
                key: key.to_string(),
            });
        }

        Ok(Self { forms: parsed })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    Text(Vec<Segment>),
    Plural {
        forms: PluralForms,
        count_name: String,
    },
}

impl Message {
    pub fn text(template: impl Into<String>) -> Result<Self, I18nError> {
        let template = template.into();
        Ok(Self::Text(tokenize_template("<message>", &template)?))
    }

    pub fn plural(forms: PluralForms) -> Self {
        let count_name = infer_count_name(&forms);
        Self::Plural { forms, count_name }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatalogValue {
    Text(String),
    Plural(BTreeMap<String, String>),
    Message(Message),
}

impl CatalogValue {
    pub fn plural<I, K, V>(forms: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self::Plural(
            forms
                .into_iter()
                .map(|(category, template)| (category.into(), template.into()))
                .collect(),
        )
    }
}

impl From<String> for CatalogValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for CatalogValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Message> for CatalogValue {
    fn from(value: Message) -> Self {
        Self::Message(value)
    }
}

impl From<PluralForms> for CatalogValue {
    fn from(value: PluralForms) -> Self {
        Self::Message(Message::plural(value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Catalog {
    locale: Locale,
    messages: HashMap<String, Message>,
}

impl Catalog {
    pub fn from_entries<I, K, V>(locale: Locale, entries: I) -> Result<Self, I18nError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<CatalogValue>,
    {
        let mut messages = HashMap::new();
        for (key, value) in entries {
            let key = key.into();
            if key.is_empty() {
                return Err(I18nError::InvalidCatalog(
                    "catalog keys must not be empty".into(),
                ));
            }
            if messages.contains_key(&key) {
                return Err(I18nError::DuplicateKey(key));
            }
            let message = match value.into() {
                CatalogValue::Text(template) => Message::Text(tokenize_template(&key, &template)?),
                CatalogValue::Plural(forms) => {
                    Message::plural(PluralForms::from_template_pairs(&key, forms)?)
                }
                CatalogValue::Message(message) => message,
            };
            messages.insert(key, message);
        }

        Ok(Self { locale, messages })
    }

    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    pub fn get(&self, key: &str) -> Option<&Message> {
        self.messages.get(key)
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    #[cfg(feature = "json")]
    pub fn from_json(locale: Locale, source: impl AsRef<str>) -> Result<Self, I18nError> {
        let value: serde_json::Value = serde_json::from_str(source.as_ref())
            .map_err(|error| I18nError::InvalidCatalog(format!("invalid JSON: {error}")))?;
        let object = value.as_object().ok_or_else(|| {
            I18nError::InvalidCatalog("the catalog root must be a JSON object".into())
        })?;

        let mut leaves = HashMap::new();
        let mut objects = std::collections::HashSet::new();
        for (key, value) in object {
            visit_json_value(key, value, &mut leaves, &mut objects)?;
        }
        Self::from_entries(locale, leaves)
    }
}

fn infer_count_name(forms: &PluralForms) -> String {
    let mut common: Option<BTreeSet<String>> = None;
    for segments in forms.forms.values() {
        let names = segments
            .iter()
            .filter_map(|segment| match segment {
                Segment::Argument(name) => Some(name.clone()),
                Segment::Literal(_) => None,
            })
            .collect::<BTreeSet<_>>();
        common = Some(match common {
            Some(existing) => existing.intersection(&names).cloned().collect(),
            None => names,
        });
    }

    if common.as_ref().is_some_and(|names| names.contains("count")) {
        "count".into()
    } else if let Some(names) = common
        && names.len() == 1
    {
        names.into_iter().next().expect("set has one name")
    } else {
        "count".into()
    }
}

fn tokenize_template(key: &str, template: &str) -> Result<Vec<Segment>, I18nError> {
    let mut segments = Vec::new();
    let mut literal_start = 0;
    let bytes = template.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'{' => {
                if literal_start < index {
                    segments.push(Segment::Literal(template[literal_start..index].to_string()));
                }
                let close = template[index + 1..]
                    .find('}')
                    .map(|offset| index + 1 + offset)
                    .ok_or_else(|| I18nError::InvalidMessage {
                        key: key.to_string(),
                        reason: "placeholder is missing a closing brace".into(),
                    })?;
                let name = &template[index + 1..close];
                if name.is_empty() {
                    return Err(I18nError::InvalidMessage {
                        key: key.to_string(),
                        reason: "placeholder name must not be empty".into(),
                    });
                }
                if !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    || name
                        .bytes()
                        .next()
                        .is_some_and(|byte| byte.is_ascii_digit())
                {
                    return Err(I18nError::InvalidMessage {
                        key: key.to_string(),
                        reason: format!("invalid placeholder name '{name}'"),
                    });
                }
                segments.push(Segment::Argument(name.to_string()));
                index = close + 1;
                literal_start = index;
            }
            b'}' => {
                return Err(I18nError::InvalidMessage {
                    key: key.to_string(),
                    reason: "placeholder has an unexpected closing brace".into(),
                });
            }
            _ => index += 1,
        }
    }

    if literal_start < template.len() {
        segments.push(Segment::Literal(template[literal_start..].to_string()));
    }
    Ok(segments)
}

#[cfg(feature = "json")]
fn visit_json_value(
    path: &str,
    value: &serde_json::Value,
    leaves: &mut HashMap<String, CatalogValue>,
    objects: &mut std::collections::HashSet<String>,
) -> Result<(), I18nError> {
    if let Some(object) = value.as_object() {
        if object
            .keys()
            .any(|key| PluralCategory::from_name(key).is_some())
        {
            if !object.contains_key("other") {
                return Err(I18nError::MissingOther {
                    key: path.to_string(),
                });
            }
            let mut forms = BTreeMap::new();
            for (category, value) in object {
                if PluralCategory::from_name(category).is_none() {
                    return Err(I18nError::InvalidCatalog(format!(
                        "invalid plural category '{category}' for '{path}'"
                    )));
                }
                let template = value.as_str().ok_or_else(|| {
                    I18nError::InvalidCatalog(format!(
                        "plural form '{path}.{category}' must be a string"
                    ))
                })?;
                forms.insert(category.clone(), template.to_string());
            }
            insert_json_leaf(path, CatalogValue::Plural(forms), leaves, objects)?;
            return Ok(());
        }

        if !path.is_empty() {
            if leaves.contains_key(path) || has_leaf_ancestor(path, leaves) {
                return Err(json_path_collision(path));
            }
            objects.insert(path.to_string());
        }
        for (key, child) in object {
            if key.is_empty() {
                return Err(I18nError::InvalidCatalog(
                    "catalog object keys must not be empty".into(),
                ));
            }
            let child_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            visit_json_value(&child_path, child, leaves, objects)?;
        }
        return Ok(());
    }

    if value.as_str().is_none() {
        return Err(I18nError::InvalidCatalog(format!(
            "message '{path}' must be a string or plural object"
        )));
    }
    insert_json_leaf(
        path,
        CatalogValue::Text(value.as_str().expect("checked above").to_string()),
        leaves,
        objects,
    )
}

#[cfg(feature = "json")]
fn insert_json_leaf(
    path: &str,
    value: CatalogValue,
    leaves: &mut HashMap<String, CatalogValue>,
    objects: &std::collections::HashSet<String>,
) -> Result<(), I18nError> {
    if path.is_empty()
        || leaves.contains_key(path)
        || objects.contains(path)
        || has_leaf_ancestor(path, leaves)
    {
        return Err(json_path_collision(path));
    }
    leaves.insert(path.to_string(), value);
    Ok(())
}

#[cfg(feature = "json")]
fn has_leaf_ancestor(path: &str, leaves: &HashMap<String, CatalogValue>) -> bool {
    let mut end = path.len();
    while let Some(separator) = path[..end].rfind('.') {
        if leaves.contains_key(&path[..separator]) {
            return true;
        }
        end = separator;
    }
    false
}

#[cfg(feature = "json")]
fn json_path_collision(path: &str) -> I18nError {
    I18nError::InvalidCatalog(format!(
        "catalog path '{path}' is both a message and an object"
    ))
}
