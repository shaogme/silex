use crate::Locale;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntlError {
    InvalidValue(String),
    JavaScript(String),
    Unsupported(&'static str),
}

impl Display for IntlError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidValue(value) => write!(f, "invalid Intl value: {value}"),
            Self::JavaScript(reason) => write!(f, "Intl operation failed: {reason}"),
            Self::Unsupported(formatter) => write!(f, "Intl formatter is unsupported: {formatter}"),
        }
    }
}

impl std::error::Error for IntlError {}

#[derive(Clone, Debug)]
pub struct NumberFormat {
    locale: Locale,
}

pub type NumberFormatter = NumberFormat;

impl NumberFormat {
    pub fn new(locale: impl Into<Locale>) -> Self {
        Self {
            locale: locale.into(),
        }
    }

    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    pub fn format(&self, value: f64) -> Result<String, IntlError> {
        if !value.is_finite() {
            return Err(IntlError::InvalidValue("number must be finite".to_string()));
        }

        #[cfg(target_arch = "wasm32")]
        {
            return wasm::format_number(&self.locale, value);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            format_number_host(&self.locale, value)
        }
    }
}

#[derive(Clone, Debug)]
pub struct DateTimeFormat {
    locale: Locale,
}

pub type DateTimeFormatter = DateTimeFormat;

impl DateTimeFormat {
    pub fn new(locale: impl Into<Locale>) -> Self {
        Self {
            locale: locale.into(),
        }
    }

    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    /// Formats a Unix timestamp expressed in milliseconds.
    pub fn format(&self, timestamp_millis: f64) -> Result<String, IntlError> {
        if !timestamp_millis.is_finite() {
            return Err(IntlError::InvalidValue(
                "timestamp must be finite".to_string(),
            ));
        }

        #[cfg(target_arch = "wasm32")]
        {
            return wasm::format_date_time(&self.locale, timestamp_millis);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            format_date_time_host(&self.locale, timestamp_millis)
        }
    }

    pub fn format_timestamp(&self, timestamp_millis: f64) -> Result<String, IntlError> {
        self.format(timestamp_millis)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Intl;

impl Intl {
    pub const fn new() -> Self {
        Self
    }

    pub fn number(locale: impl Into<Locale>) -> NumberFormat {
        NumberFormat::new(locale)
    }

    pub fn date_time(locale: impl Into<Locale>) -> DateTimeFormat {
        DateTimeFormat::new(locale)
    }

    pub fn number_format(locale: impl Into<Locale>) -> NumberFormat {
        Self::number(locale)
    }

    pub fn date_time_format(locale: impl Into<Locale>) -> DateTimeFormat {
        Self::date_time(locale)
    }

    pub fn format_number(locale: impl Into<Locale>, value: f64) -> Result<String, IntlError> {
        Self::number(locale).format(value)
    }

    pub fn format_date_time(
        locale: impl Into<Locale>,
        timestamp_millis: f64,
    ) -> Result<String, IntlError> {
        Self::date_time(locale).format(timestamp_millis)
    }
}

pub fn format_number(locale: &Locale, value: f64) -> Result<String, IntlError> {
    NumberFormat::new(locale.clone()).format(value)
}

pub fn format_date_time(locale: &Locale, timestamp_millis: f64) -> Result<String, IntlError> {
    DateTimeFormat::new(locale.clone()).format(timestamp_millis)
}

#[cfg(not(target_arch = "wasm32"))]
fn format_number_host(locale: &Locale, value: f64) -> Result<String, IntlError> {
    let raw = value.to_string();
    if raw.contains('e') || raw.contains('E') {
        return Ok(raw);
    }

    let (group_separator, decimal_separator) = number_separators(locale);
    let (sign, unsigned) = raw
        .strip_prefix('-')
        .map_or(("", raw.as_str()), |value| ("-", value));
    let (integer, fraction) = unsigned
        .split_once('.')
        .map_or((unsigned, ""), |(integer, fraction)| (integer, fraction));
    let grouped = group_digits(integer, group_separator);

    if fraction.is_empty() {
        Ok(format!("{sign}{grouped}"))
    } else {
        Ok(format!("{sign}{grouped}{decimal_separator}{fraction}"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn number_separators(locale: &Locale) -> (&'static str, &'static str) {
    match locale.language() {
        "de" | "da" | "el" | "es" | "it" | "nl" | "pt" | "tr" => (".", ","),
        "fr" => ("\u{202f}", ","),
        "cs" | "pl" | "ru" | "uk" => ("\u{a0}", ","),
        _ => (",", "."),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn group_digits(value: &str, separator: &str) -> String {
    if value.len() <= 3 {
        return value.to_string();
    }

    let first_group_length = value.len() % 3;
    let first_group_length = if first_group_length == 0 {
        3
    } else {
        first_group_length
    };
    let mut output = String::with_capacity(value.len() + value.len() / 3 * separator.len());
    output.push_str(&value[..first_group_length]);
    let mut index = first_group_length;
    while index < value.len() {
        output.push_str(separator);
        output.push_str(&value[index..index + 3]);
        index += 3;
    }
    output
}

#[cfg(not(target_arch = "wasm32"))]
fn format_date_time_host(_locale: &Locale, timestamp_millis: f64) -> Result<String, IntlError> {
    let seconds = (timestamp_millis / 1_000.0).floor() as i64;
    let days = seconds.div_euclid(86_400);
    let seconds_in_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date_from_days(days);
    let hour = seconds_in_day / 3_600;
    let minute = seconds_in_day % 3_600 / 60;
    let second = seconds_in_day % 60;
    Ok(format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC"
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn civil_date_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted / 146_097
    } else {
        (shifted - 146_096) / 146_097
    };
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::IntlError;
    use crate::Locale;
    use js_sys::{Array, Function, Reflect};
    use wasm_bindgen::{JsCast, JsValue};

    pub fn format_number(locale: &Locale, value: f64) -> Result<String, IntlError> {
        let formatter = construct_formatter("NumberFormat", locale)?;
        format_value(&formatter, JsValue::from_f64(value))
    }

    pub fn format_date_time(locale: &Locale, timestamp_millis: f64) -> Result<String, IntlError> {
        let formatter = construct_formatter("DateTimeFormat", locale)?;
        let date = js_sys::Date::new(&JsValue::from_f64(timestamp_millis));
        format_value(&formatter, date.into())
    }

    fn construct_formatter(name: &str, locale: &Locale) -> Result<JsValue, IntlError> {
        let global = js_sys::global();
        let intl = Reflect::get(&global, &JsValue::from_str("Intl")).map_err(js_error)?;
        let constructor = Reflect::get(&intl, &JsValue::from_str(name))
            .map_err(js_error)?
            .dyn_into::<Function>()
            .map_err(|_| IntlError::JavaScript(format!("Intl.{name} is not a constructor")))?;
        let arguments = Array::new();
        arguments.push(&JsValue::from_str(locale.as_str()));
        Reflect::construct(&constructor, &arguments).map_err(js_error)
    }

    fn format_value(formatter: &JsValue, value: JsValue) -> Result<String, IntlError> {
        let format = Reflect::get(formatter, &JsValue::from_str("format"))
            .map_err(js_error)?
            .dyn_into::<Function>()
            .map_err(|_| IntlError::JavaScript("Intl formatter has no format method".into()))?;
        format
            .call1(formatter, &value)
            .map_err(js_error)?
            .as_string()
            .ok_or_else(|| IntlError::JavaScript("Intl format did not return a string".into()))
    }

    fn js_error(value: JsValue) -> IntlError {
        IntlError::JavaScript(
            value
                .as_string()
                .unwrap_or_else(|| "unknown JavaScript error".to_string()),
        )
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn host_number_format_is_deterministic() {
        assert_eq!(
            format_number(&Locale::new("en-US"), 1_234_567.5).expect("number formats"),
            "1,234,567.5"
        );
        assert_eq!(
            format_number(&Locale::new("de-DE"), 1_234_567.5).expect("number formats"),
            "1.234.567,5"
        );
    }

    #[test]
    fn host_date_format_is_deterministic() {
        assert_eq!(
            format_date_time(&Locale::new("en-US"), 0.0).expect("date formats"),
            "1970-01-01 00:00:00 UTC"
        );
        assert_eq!(
            format_date_time(&Locale::new("en-US"), -1.0).expect("date formats"),
            "1969-12-31 23:59:59 UTC"
        );
    }
}
