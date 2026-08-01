use crate::I18nError;
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Locale {
    value: String,
}

impl Locale {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        Self::parse(&value).unwrap_or_else(|error| panic!("{error}"))
    }

    pub fn parse(value: impl AsRef<str>) -> Result<Self, I18nError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(I18nError::InvalidLocale("locale must not be empty".into()));
        }
        if value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        {
            return Err(I18nError::InvalidLocale(
                "locale must not contain whitespace or control characters".into(),
            ));
        }

        let value = value.replace('_', "-");
        let subtags: Vec<&str> = value.split('-').collect();
        if subtags.iter().any(|subtag| subtag.is_empty()) {
            return Err(I18nError::InvalidLocale(
                "locale must not contain empty subtags".into(),
            ));
        }
        if subtags[0].is_empty()
            || !subtags[0].bytes().all(|byte| byte.is_ascii_alphabetic())
            || subtags[0].len() > 8
        {
            return Err(I18nError::InvalidLocale(
                "the language subtag must contain 1 to 8 ASCII letters".into(),
            ));
        }
        if subtags
            .iter()
            .any(|subtag| !subtag.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        {
            return Err(I18nError::InvalidLocale(
                "locale subtags must contain only ASCII letters and digits".into(),
            ));
        }

        let normalized = subtags
            .iter()
            .enumerate()
            .map(|(index, subtag)| normalize_subtag(index, subtag))
            .collect::<Vec<_>>()
            .join("-");

        Ok(Self { value: normalized })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn language(&self) -> &str {
        self.value
            .split('-')
            .next()
            .expect("Locale always contains a language subtag")
    }

    pub fn fallback_chain(&self) -> impl Iterator<Item = Locale> {
        let subtags: Vec<&str> = self.value.split('-').collect();
        let mut chain = Vec::with_capacity(subtags.len());
        for length in (1..=subtags.len()).rev() {
            chain.push(Self {
                value: subtags[..length].join("-"),
            });
        }
        chain.into_iter()
    }
}

fn normalize_subtag(index: usize, subtag: &str) -> String {
    if index == 0 {
        return subtag.to_ascii_lowercase();
    }
    if subtag.len() == 4 && subtag.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        let mut chars = subtag.chars();
        let first = chars
            .next()
            .expect("a four-character subtag is not empty")
            .to_ascii_uppercase();
        return format!("{first}{}", chars.as_str().to_ascii_lowercase());
    }
    if (subtag.len() == 2 && subtag.bytes().all(|byte| byte.is_ascii_alphabetic()))
        || (subtag.len() == 3 && subtag.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return subtag.to_ascii_uppercase();
    }
    subtag.to_string()
}

impl AsRef<str> for Locale {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Display for Locale {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Locale {
    type Err = I18nError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}
