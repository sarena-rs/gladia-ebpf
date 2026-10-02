use std::{collections::HashSet, fs, path::Path};

use syn::{Expr, ExprLit, Lit, Macro, Token, punctuated::Punctuated, visit::Visit};

use crate::constants::{RUST_EXTENSION, TAIL_CALL_MACRO_NAME};

pub fn visit_dir(dir: &Path, calls: &mut HashSet<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();

        if path.is_dir() {
            visit_dir(&path, calls)?;
        } else if path.extension().is_some_and(|x| x == RUST_EXTENSION) {
            visit_file(&path, calls)?;
        }
    }
    Ok(())
}

fn visit_file(path: &Path, calls: &mut HashSet<String>) -> std::io::Result<()> {
    println!("cargo:rerun-if-changed={}", path.display());

    let source = fs::read_to_string(path)?;
    let file = syn::parse_file(&source)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
    let mut visitor = CallVisitor {
        path,
        calls: HashSet::new(),
    };
    visitor.visit_file(&file);
    calls.extend(visitor.calls);
    Ok(())
}

struct CallVisitor<'a> {
    path: &'a Path,
    calls: HashSet<String>,
}

impl<'ast> Visit<'ast> for CallVisitor<'_> {
    fn visit_macro(&mut self, m: &'ast Macro) {
        let is_tail_call = m
            .path
            .segments
            .last()
            .is_some_and(|seg| seg.ident == TAIL_CALL_MACRO_NAME);

        if is_tail_call {
            let fail = |reason: &str| -> ! {
                panic!(
                    "{}: invalid `tail_call!({})`: {reason}",
                    self.path.display(),
                    m.tokens
                )
            };

            let args = m
                .parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
                .unwrap_or_else(|e| fail(&e.to_string()));

            match args.iter().nth(1) {
                Some(Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                })) => self.calls.insert(s.value()),
                Some(_) => fail("second argument must be a string literal"),
                None => fail("expected two arguments: `tail_call!(&ctx, \"name\")`"),
            };
        }

        syn::visit::visit_macro(self, m);
    }
}
