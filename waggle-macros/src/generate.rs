use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn expand(_item: TokenStream) -> TokenStream {
    quote! {
        #[allow(unused_macros)]
        include!(concat!(env!("OUT_DIR"), "/__waggle_tail_calls.rs"));
    }
}
