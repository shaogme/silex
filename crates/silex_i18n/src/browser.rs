use crate::Locale;
use silex_core::reactivity::Effect;
use silex_core::traits::RxGet;

/// The direction used by a locale when updating document metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextDirection {
    Ltr,
    Rtl,
}

impl TextDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
        }
    }
}

/// Resolves an ordered list of requested locales against available catalogs.
pub fn resolve_requested_locale(
    requested: impl IntoIterator<Item = Locale>,
    available: &[Locale],
    fallback: &Locale,
) -> Locale {
    let requested = requested.into_iter().collect::<Vec<_>>();

    for requested in &requested {
        if let Some(locale) = available.iter().find(|locale| *locale == requested) {
            return locale.clone();
        }
    }

    for requested in &requested {
        if let Some(locale) = available
            .iter()
            .find(|locale| locale.language() == requested.language())
        {
            return locale.clone();
        }
    }

    for requested in &requested {
        for candidate in requested.fallback_chain().skip(1) {
            if let Some(locale) = available.iter().find(|locale| *locale == &candidate) {
                return locale.clone();
            }
        }
    }

    fallback.clone()
}

/// Reads valid locale candidates from `navigator.languages`.
pub fn navigator_languages() -> Vec<Locale> {
    let Some(window) = web_sys::window() else {
        return Vec::new();
    };

    let navigator = window.navigator();
    let mut locales = Vec::new();
    for value in navigator.languages().iter() {
        if let Some(raw) = value.as_string()
            && let Ok(locale) = raw.parse()
        {
            locales.push(locale);
        }
    }

    if locales.is_empty()
        && let Some(raw) = navigator.language()
        && let Ok(locale) = raw.parse()
    {
        locales.push(locale);
    }

    locales
}

/// Resolves the browser's preferred locale against available catalogs.
pub fn detect_browser_locale(available: &[Locale], fallback: &Locale) -> Locale {
    resolve_requested_locale(navigator_languages(), available, fallback)
}

/// Returns the conventional text direction for a locale.
pub fn locale_direction(locale: &Locale) -> TextDirection {
    match locale.language() {
        "ar" | "dv" | "fa" | "he" | "ku" | "ps" | "ur" | "yi" => TextDirection::Rtl,
        _ => TextDirection::Ltr,
    }
}

/// Keeps the document's `lang` and `dir` attributes in sync with the store.
pub fn sync_document_metadata(store: crate::I18nStore) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };

    let locale = store.locale();
    Effect::new(move |_| {
        let locale = locale.get();
        if let Some(root) = document.document_element() {
            let _ = root.set_attribute("lang", locale.as_str());
            let _ = root.set_attribute("dir", locale_direction(&locale).as_str());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locale(value: &str) -> Locale {
        Locale::new(value)
    }

    #[test]
    fn resolves_exact_then_language_then_fallback_chain() {
        let available = [locale("en-US"), locale("zh-Hant"), locale("zh")];

        assert_eq!(
            resolve_requested_locale([locale("zh-Hant-TW")], &available, &locale("en-US")),
            locale("zh-Hant")
        );
        assert_eq!(
            resolve_requested_locale([locale("en-GB")], &available, &locale("zh")),
            locale("en-US")
        );
        assert_eq!(
            resolve_requested_locale([locale("fr")], &available, &locale("zh")),
            locale("zh")
        );
    }

    #[test]
    fn identifies_rtl_languages() {
        assert_eq!(locale_direction(&locale("ar-EG")), TextDirection::Rtl);
        assert_eq!(locale_direction(&locale("en-US")), TextDirection::Ltr);
    }
}
