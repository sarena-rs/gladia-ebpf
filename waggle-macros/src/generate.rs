use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemFn;

pub(crate) fn expand(item: TokenStream) -> TokenStream {
    quote! {
        #[used]
        #[unsafe(link_section = ".test_entry_calls")]
        static __TEST_ENTRY_CALLS: TestEntryHeader<3> = TestEntryHeader {
            version: 1,
            file_name: make_name(b"main.rs"),
            count: 3u32,
            size: core::mem::size_of::<TestEntryCall>() as u32,
            entries: [
                TestEntryCall {
                    index: 0u32,
                    name: make_name(b"filter_ipv4"),
                },
                TestEntryCall {
                    index: 1u32,
                    name: make_name(b"filter_tcp"),
                },
                TestEntryCall {
                    index: 2u32,
                    name: make_name(b"filter_udp"),
                },
            ],
        };
    }
}

// include_generated!();
// expands to:
// include!(concat!(env!("OUT_DIR"), "/my_framework.rs"));
