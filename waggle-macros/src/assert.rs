use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemFn;

use crate::common::{ProgramAttrs, ProgramMode, outer_fn_ident, passthrough_xdp};

pub(crate) fn expand(attrs: TokenStream, item: TokenStream) -> TokenStream {
    match AssertProgram::parse(attrs.into(), item.into()) {
        Ok(prog) => prog.expand(),
        Err(err) => err.to_compile_error(),
    }
    .into()
}

pub(crate) struct AssertProgram {
    item: ItemFn,
    mode: ProgramMode,
    name: String,
}

impl AssertProgram {
    pub(crate) fn parse(attrs: TokenStream, item: TokenStream) -> syn::Result<Self> {
        let item: ItemFn = syn::parse2(item)?;
        let ProgramAttrs { mode, name } = syn::parse2(attrs)?;
        Ok(Self { item, mode, name })
    }

    pub(crate) fn expand(&self) -> TokenStream {
        match self.mode {
            ProgramMode::Tc => self.expand_tc(),
            ProgramMode::Xdp => passthrough_xdp("assert", &self.name, &self.item),
        }
    }

    fn expand_tc(&self) -> TokenStream {
        let Self { item, name, .. } = self;
        let ItemFn {
            attrs, sig, block, ..
        } = item;
        let outer_fn = outer_fn_ident("assert", name);
        let fn_name = &sig.ident;
        let stmts = &block.stmts;
        quote! {
            #[unsafe(no_mangle)]
            #[unsafe(link_section = "classifier")]
            pub fn #outer_fn(ctx: *mut ::waggle_ebpf::__private::aya_ebpf::bindings::__sk_buff) -> i32 {
                let ctx = unsafe { ::core::ptr::NonNull::new_unchecked(ctx) };
                let tc_ctx = ::waggle_ebpf::__private::aya_ebpf::programs::TcContext::new(ctx);
                let mut test_suite = match ::waggle_ebpf::suite::TestSuite::new(#name, ::core::file!()) {
                    None => return ::waggle_ebpf::wire::TestStatus::FrameworkError as i32,
                    Some(s) => s,
                };

                #fn_name(tc_ctx, &mut test_suite);

                if test_suite.writer.write_u8(::waggle_ebpf::wire::Tag::TestStatus, test_suite.status() as u8).is_none() {
                    return ::waggle_ebpf::wire::TestStatus::FrameworkError as i32;
                }

                test_suite.status() as i32
            }

            #(#attrs)*
            #sig {
                loop {
                    #(#stmts)*
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn test_tc_expand() {
        let prog = AssertProgram::parse(
            parse_quote! { tc, "my_test" },
            parse_quote! {
                fn prog(ctx: ::aya_ebpf::programs::TcContext, t: &mut TestSuite) {
                    assert_test!(t, 1 + 1 == 2);
                }
            },
        )
        .unwrap();
        let expanded = prog.expand();
        let expected = quote! {
            #[unsafe(no_mangle)]
            #[unsafe(link_section = "classifier")]
            pub fn __test_fw_assert_my_test(ctx: *mut ::waggle_ebpf::__private::aya_ebpf::bindings::__sk_buff) -> i32 {
                let ctx = unsafe { ::core::ptr::NonNull::new_unchecked(ctx) };
                let tc_ctx = ::waggle_ebpf::__private::aya_ebpf::programs::TcContext::new(ctx);
                let mut test_suite = match ::waggle_ebpf::suite::TestSuite::new("my_test", ::core::file!()) {
                    None => return ::waggle_ebpf::wire::TestStatus::FrameworkError as i32,
                    Some(s) => s,
                };

                prog(tc_ctx, &mut test_suite);

                if test_suite.writer.write_u8(::waggle_ebpf::wire::Tag::TestStatus, test_suite.status() as u8).is_none() {
                    return ::waggle_ebpf::wire::TestStatus::FrameworkError as i32;
                }

                test_suite.status() as i32
            }

            fn prog(ctx: ::aya_ebpf::programs::TcContext, t: &mut TestSuite) {
                loop {
                    assert_test!(t, 1 + 1 == 2);
                    break;
                }
            }
        };
        assert_eq!(expected.to_string(), expanded.to_string());
    }

    #[test]
    fn test_xdp_expand_with_order() {
        let prog = AssertProgram::parse(
            parse_quote! { xdp, "firewall_test" },
            parse_quote! {
                fn check(ctx: ::aya_ebpf::programs::XdpContext) -> ::waggle_ebpf::wire::TestStatus {
                    ::waggle_ebpf::wire::TestStatus::Pass
                }
            },
        )
        .unwrap();
        assert_eq!(prog.name, "firewall_test");
        let expanded = prog.expand();
        let expected = quote! {
            #[unsafe(no_mangle)]
            #[unsafe(link_section = "xdp")]
            pub fn __test_fw_assert_firewall_test(ctx: *mut ::waggle_ebpf::__private::aya_ebpf::bindings::xdp_md) -> u32 {
                check(::waggle_ebpf::__private::aya_ebpf::programs::XdpContext::new(ctx)) as u32
            }

            fn check(ctx: ::aya_ebpf::programs::XdpContext) -> ::waggle_ebpf::wire::TestStatus {
                ::waggle_ebpf::wire::TestStatus::Pass
            }
        };
        assert_eq!(expected.to_string(), expanded.to_string());
    }
}
