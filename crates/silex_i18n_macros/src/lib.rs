#![allow(linker_messages)]
#![doc = "Procedural macros for Silex internationalization."]

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod i18n_keys;

#[proc_macro_derive(I18nKeys, attributes(i18n))]
pub fn derive_i18n_keys(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match i18n_keys::derive_i18n_keys(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}
