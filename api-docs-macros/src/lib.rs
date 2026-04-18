//! OpenAPI 辅助宏 crate 入口。
#![deny(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

extern crate proc_macro;

use proc_macro::TokenStream;

#[cfg(all(feature = "openapi", debug_assertions))]
use quote::quote;

#[cfg(all(feature = "openapi", debug_assertions))]
#[proc_macro_attribute]
pub fn path(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = proc_macro2::TokenStream::from(attr);
    let item = proc_macro2::TokenStream::from(item);

    quote! {
        #[utoipa::path(#attr)]
        #item
    }
    .into()
}

#[cfg(not(all(feature = "openapi", debug_assertions)))]
#[proc_macro_attribute]
pub fn path(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
