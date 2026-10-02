mod act;
mod arrange;
mod assert;
mod common;
mod generate;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn arrange(attrs: TokenStream, item: TokenStream) -> TokenStream {
    arrange::expand(attrs.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn act(attrs: TokenStream, item: TokenStream) -> TokenStream {
    act::expand(attrs.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn assert(attrs: TokenStream, item: TokenStream) -> TokenStream {
    assert::expand(attrs.into(), item.into()).into()
}

#[proc_macro]
pub fn include_generated(item: TokenStream) -> TokenStream {
    generate::expand(item.into()).into()
}
