use crate::Locale;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Two => "two",
            Self::Few => "few",
            Self::Many => "many",
            Self::Other => "other",
        }
    }

    pub(crate) fn from_name(value: &str) -> Option<Self> {
        match value {
            "zero" => Some(Self::Zero),
            "one" => Some(Self::One),
            "two" => Some(Self::Two),
            "few" => Some(Self::Few),
            "many" => Some(Self::Many),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

pub fn plural_category(locale: &Locale, number: f64) -> PluralCategory {
    if !number.is_finite() {
        return PluralCategory::Other;
    }

    let number = number.abs();
    let integer = number.trunc() as u64;
    let is_integer = number.fract() == 0.0;
    let language = locale.language();

    match language {
        "ar" => {
            if is_integer && integer == 0 {
                PluralCategory::Zero
            } else if is_integer && integer == 1 {
                PluralCategory::One
            } else if is_integer && integer == 2 {
                PluralCategory::Two
            } else if is_integer && (3..=10).contains(&(integer % 100)) {
                PluralCategory::Few
            } else if is_integer && (11..=99).contains(&(integer % 100)) {
                PluralCategory::Many
            } else {
                PluralCategory::Other
            }
        }
        "ru" | "uk" | "be" => {
            if !is_integer {
                PluralCategory::Other
            } else if integer % 10 == 1 && integer % 100 != 11 {
                PluralCategory::One
            } else if (2..=4).contains(&(integer % 10)) && !(12..=14).contains(&(integer % 100)) {
                PluralCategory::Few
            } else if integer.is_multiple_of(10)
                || (5..=9).contains(&(integer % 10))
                || (11..=14).contains(&(integer % 100))
            {
                PluralCategory::Many
            } else {
                PluralCategory::Other
            }
        }
        "pl" => {
            if !is_integer {
                PluralCategory::Other
            } else if integer == 1 {
                PluralCategory::One
            } else if (2..=4).contains(&(integer % 10)) && !(12..=14).contains(&(integer % 100)) {
                PluralCategory::Few
            } else {
                PluralCategory::Many
            }
        }
        "cs" | "sk" => {
            if is_integer && integer == 1 {
                PluralCategory::One
            } else if is_integer && (2..=4).contains(&integer) {
                PluralCategory::Few
            } else {
                PluralCategory::Many
            }
        }
        "sl" => {
            if !is_integer {
                PluralCategory::Other
            } else {
                match integer % 100 {
                    1 => PluralCategory::One,
                    2 => PluralCategory::Two,
                    3..=4 => PluralCategory::Few,
                    _ => PluralCategory::Other,
                }
            }
        }
        "ro" => {
            if is_integer && integer == 1 {
                PluralCategory::One
            } else if !is_integer || integer == 0 || (1..=19).contains(&(integer % 100)) {
                PluralCategory::Few
            } else {
                PluralCategory::Other
            }
        }
        "lt" => {
            if !is_integer {
                PluralCategory::Other
            } else if integer % 10 == 1 && !(11..=19).contains(&(integer % 100)) {
                PluralCategory::One
            } else if (2..=9).contains(&(integer % 10)) && !(11..=19).contains(&(integer % 100)) {
                PluralCategory::Few
            } else {
                PluralCategory::Other
            }
        }
        "lv" => {
            if !is_integer || integer == 0 {
                PluralCategory::Zero
            } else if integer % 10 == 1 && integer % 100 != 11 {
                PluralCategory::One
            } else {
                PluralCategory::Other
            }
        }
        "ga" => {
            if is_integer && integer == 1 {
                PluralCategory::One
            } else if is_integer && integer == 2 {
                PluralCategory::Two
            } else if is_integer && (3..=6).contains(&integer) {
                PluralCategory::Few
            } else if is_integer && (7..=10).contains(&integer) {
                PluralCategory::Many
            } else {
                PluralCategory::Other
            }
        }
        "cy" => match integer {
            0 => PluralCategory::Zero,
            1 => PluralCategory::One,
            2 => PluralCategory::Two,
            3 => PluralCategory::Few,
            6 => PluralCategory::Many,
            _ => PluralCategory::Other,
        },
        "fr" | "pt" => {
            if !is_integer || integer <= 1 {
                PluralCategory::One
            } else {
                PluralCategory::Other
            }
        }
        _ => {
            if is_integer && integer == 1 {
                PluralCategory::One
            } else {
                PluralCategory::Other
            }
        }
    }
}
