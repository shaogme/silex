use crate::{I18nStore, Locale};
use silex_core::{EffectHandle, EffectPhase, RxGet, SilexError, SilexResult};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::JsValue;

const METADATA_STACK_PROPERTY: &str = "__silex_i18n_metadata_stack";
const RECORD_ID: &str = "id";
const RECORD_ACTIVE: &str = "active";
const RECORD_PREVIOUS_LANG: &str = "previous_lang";
const RECORD_PREVIOUS_DIR: &str = "previous_dir";
const RECORD_LAST_LANG: &str = "last_lang";
const RECORD_LAST_DIR: &str = "last_dir";
const RECORD_DESIRED_LANG: &str = "desired_lang";
const RECORD_DESIRED_DIR: &str = "desired_dir";

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
pub(crate) fn sync_document_metadata<'owner>(
    store: I18nStore<'owner>,
) -> SilexResult<EffectHandle<'owner>> {
    let owner = store.owner();
    #[cfg(target_arch = "wasm32")]
    let root = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element());
    #[cfg(not(target_arch = "wasm32"))]
    let root: Option<web_sys::Element> = None;
    let Some(root) = root else {
        let effect = owner.effect(EffectPhase::Normal, || Ok(()), store.error_handler())?;
        effect.stop()?;
        return Ok(effect);
    };

    let previous_lang = root.get_attribute("lang");
    let previous_dir = root.get_attribute("dir");
    let active = Rc::new(Cell::new(true));
    let locale = store.locale();
    let stack_property = JsValue::from_str(METADATA_STACK_PROPERTY);
    let owner_id = JsValue::from_str(&format!("{:p}", Rc::as_ptr(&active)));
    let record = JsValue::from(js_sys::Object::new());
    set_record_value(&record, RECORD_ID, &owner_id)?;
    set_record_value(&record, RECORD_ACTIVE, &JsValue::TRUE)?;
    set_record_value(
        &record,
        RECORD_PREVIOUS_LANG,
        &optional_string_value(previous_lang.as_deref()),
    )?;
    set_record_value(
        &record,
        RECORD_PREVIOUS_DIR,
        &optional_string_value(previous_dir.as_deref()),
    )?;
    set_record_value(
        &record,
        RECORD_LAST_LANG,
        &optional_string_value(previous_lang.as_deref()),
    )?;
    set_record_value(
        &record,
        RECORD_LAST_DIR,
        &optional_string_value(previous_dir.as_deref()),
    )?;
    set_record_value(
        &record,
        RECORD_DESIRED_LANG,
        &optional_string_value(previous_lang.as_deref()),
    )?;
    set_record_value(
        &record,
        RECORD_DESIRED_DIR,
        &optional_string_value(previous_dir.as_deref()),
    )?;
    // Keep ownership history separate from lang/dir so equal values stay distinguishable.
    let stack = metadata_stack(&root, &stack_property)?.unwrap_or_default();
    stack.push(&record);
    js_sys::Reflect::set(root.as_ref(), &stack_property, &stack).map_err(SilexError::fatal)?;

    let active_for_effect = active.clone();
    let root_for_effect = root.clone();
    let stack_property_for_effect = stack_property.clone();
    let owner_id_for_effect = owner_id.clone();
    let record_for_effect = record.clone();
    let error_handler = store.error_handler();
    let effect = owner.effect(
        EffectPhase::Normal,
        move || -> SilexResult<()> {
            if !active_for_effect.get() {
                return Ok(());
            }

            let locale = locale.get()?;
            let lang = locale.as_str().to_string();
            let dir = locale_direction(&locale).as_str().to_string();
            set_record_value(
                &record_for_effect,
                RECORD_DESIRED_LANG,
                &JsValue::from_str(&lang),
            )?;
            set_record_value(
                &record_for_effect,
                RECORD_DESIRED_DIR,
                &JsValue::from_str(&dir),
            )?;

            let Some(stack) = metadata_stack(&root_for_effect, &stack_property_for_effect)? else {
                return Ok(());
            };
            if stack.length() == 0
                || record_value(&stack.get(stack.length() - 1), RECORD_ID) != owner_id_for_effect
            {
                return Ok(());
            }

            apply_record(&root_for_effect, &record_for_effect, true, true)
        },
        error_handler,
    )?;

    let root_for_cleanup = root;
    let stack_property_for_cleanup = stack_property;
    let owner_id_for_cleanup = owner_id;
    owner.on_cleanup(
        move || -> SilexResult<()> {
            if !active.replace(false) {
                return Ok(());
            }

            let Some(stack) = metadata_stack(&root_for_cleanup, &stack_property_for_cleanup)?
            else {
                return Ok(());
            };
            let Some(index) = find_record(&stack, &owner_id_for_cleanup) else {
                return Ok(());
            };
            let record = stack.get(index);
            set_record_value(&record, RECORD_ACTIVE, &JsValue::FALSE)?;
            let mut controlled = (false, false);
            if index == stack.length() - 1 {
                controlled = restore_record(&root_for_cleanup, &record)?;
            } else {
                let next = stack.get(index + 1);
                set_record_value(
                    &next,
                    RECORD_PREVIOUS_LANG,
                    &record_value(&record, RECORD_PREVIOUS_LANG),
                )?;
                set_record_value(
                    &next,
                    RECORD_PREVIOUS_DIR,
                    &record_value(&record, RECORD_PREVIOUS_DIR),
                )?;
            }
            for position in index..stack.length().saturating_sub(1) {
                stack.set(position, stack.get(position + 1));
            }
            stack.pop();
            while stack.length() > 0 {
                let next = stack.get(stack.length() - 1);
                if record_value(&next, RECORD_ACTIVE)
                    .as_bool()
                    .unwrap_or(false)
                {
                    break;
                }
                let next_controlled = restore_record(&root_for_cleanup, &next)?;
                controlled.0 &= next_controlled.0;
                controlled.1 &= next_controlled.1;
                stack.pop();
            }
            if stack.length() > 0 {
                let next = stack.get(stack.length() - 1);
                apply_record(&root_for_cleanup, &next, controlled.0, controlled.1)?;
            }

            if stack.length() == 0 {
                js_sys::Reflect::delete_property(
                    root_for_cleanup.as_ref(),
                    &stack_property_for_cleanup,
                )
                .map_err(SilexError::fatal)?;
            } else {
                js_sys::Reflect::set(
                    root_for_cleanup.as_ref(),
                    &stack_property_for_cleanup,
                    &stack,
                )
                .map_err(SilexError::fatal)?;
            }
            Ok(())
        },
        error_handler,
    )?;

    Ok(effect)
}

fn metadata_stack(
    root: &web_sys::Element,
    property: &JsValue,
) -> SilexResult<Option<js_sys::Array>> {
    let value = js_sys::Reflect::get(root.as_ref(), property).map_err(SilexError::fatal)?;
    Ok(js_sys::Array::is_array(&value).then(|| js_sys::Array::from(&value)))
}

fn find_record(stack: &js_sys::Array, id: &JsValue) -> Option<u32> {
    (0..stack.length()).find(|index| record_value(&stack.get(*index), RECORD_ID) == *id)
}

fn record_value(record: &JsValue, name: &str) -> JsValue {
    js_sys::Reflect::get(record, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED)
}

fn set_record_value(record: &JsValue, name: &str, value: &JsValue) -> SilexResult<()> {
    js_sys::Reflect::set(record, &JsValue::from_str(name), value)
        .map(|_| ())
        .map_err(SilexError::fatal)
}

fn optional_string_value(value: Option<&str>) -> JsValue {
    value.map(JsValue::from_str).unwrap_or(JsValue::UNDEFINED)
}

fn optional_attribute_value(value: JsValue) -> Option<String> {
    value.as_string()
}

fn apply_record(
    root: &web_sys::Element,
    record: &JsValue,
    apply_lang: bool,
    apply_dir: bool,
) -> SilexResult<()> {
    if apply_lang
        && let Some(lang) = optional_attribute_value(record_value(record, RECORD_DESIRED_LANG))
    {
        root.set_attribute("lang", &lang)
            .map_err(SilexError::fatal)?;
        set_record_value(record, RECORD_LAST_LANG, &JsValue::from_str(&lang))?;
    }

    if apply_dir
        && let Some(dir) = optional_attribute_value(record_value(record, RECORD_DESIRED_DIR))
    {
        root.set_attribute("dir", &dir).map_err(SilexError::fatal)?;
        set_record_value(record, RECORD_LAST_DIR, &JsValue::from_str(&dir))?;
    }
    Ok(())
}

fn restore_record(root: &web_sys::Element, record: &JsValue) -> SilexResult<(bool, bool)> {
    let current_lang = root.get_attribute("lang");
    let mut controlled_lang = false;
    if current_lang == optional_attribute_value(record_value(record, RECORD_LAST_LANG)) {
        let previous = optional_attribute_value(record_value(record, RECORD_PREVIOUS_LANG));
        restore_attribute(root, "lang", previous.as_deref())?;
        controlled_lang = true;
    }

    let current_dir = root.get_attribute("dir");
    let mut controlled_dir = false;
    if current_dir == optional_attribute_value(record_value(record, RECORD_LAST_DIR)) {
        let previous = optional_attribute_value(record_value(record, RECORD_PREVIOUS_DIR));
        restore_attribute(root, "dir", previous.as_deref())?;
        controlled_dir = true;
    }

    Ok((controlled_lang, controlled_dir))
}

fn restore_attribute(root: &web_sys::Element, name: &str, value: Option<&str>) -> SilexResult<()> {
    match value {
        Some(value) => {
            root.set_attribute(name, value).map_err(SilexError::fatal)?;
        }
        None => {
            root.remove_attribute(name).map_err(SilexError::fatal)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locale(value: &str) -> Locale {
        Locale::new(value).expect("valid locale")
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn missing_document_returns_an_inactive_effect_without_leaking_nodes() {
        let mut runtime = silex_core::Runtime::new();
        runtime
            .with_transient(|owner| {
                let handler = owner.error_handler(|_| {}).expect("error handler");
                let store = crate::I18nBuilder::new(owner, handler.view())
                    .locale(Locale::new("en-US").expect("valid locale"))
                    .build()
                    .expect("valid i18n store");
                let before = owner.runtime_snapshot().expect("runtime snapshot");
                let effect = sync_document_metadata(store).expect("metadata effect can be created");
                let after = owner.runtime_snapshot().expect("runtime snapshot");
                assert_eq!(after.nodes, before.nodes);
                assert_eq!(after.data, before.data);
                assert_eq!(after.edges, before.edges);
                assert_eq!(after.roots, before.roots);
                assert_eq!(after.cleanups, before.cleanups);
                assert_eq!(after.handlers, before.handlers);
                assert_eq!(after.queue, before.queue);
                assert!(!effect.stop().expect("inactive effect can stop"));
            })
            .expect("transient owner");
    }
}
