use std::{collections::HashSet, fs, path::Path};

use syn::{Expr, ExprLit, Lit, Macro, Token, punctuated::Punctuated, visit::Visit};

use crate::constants::{RUST_EXTENSION, TAIL_CALL_MACRO_NAME};

pub struct CallVisitorItem {
    pub calls: HashSet<String>,
    pub file_name: String,
}

/// Collects the `tail_call!` targets of every Rust file under `dir`, one item per file that
/// contains at least one call.
pub fn visit_dir(dir: &Path, calls: &mut Vec<CallVisitorItem>) -> std::io::Result<()> {
    // Cargo scans a directory recursively, so this also catches files that are added or removed.
    println!("cargo:rerun-if-changed={}", dir.display());
    walk_dir(dir, calls)
}

fn walk_dir(dir: &Path, calls: &mut Vec<CallVisitorItem>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let file_type = entry.file_type()?;
        let is_dir = if file_type.is_symlink() {
            path.is_dir()
        } else {
            file_type.is_dir()
        };

        if is_dir {
            walk_dir(&path, calls)?;
        } else if path.extension().is_some_and(|x| x == RUST_EXTENSION) {
            walk_file(&path, calls)?;
        }
    }
    Ok(())
}

fn walk_file(path: &Path, calls: &mut Vec<CallVisitorItem>) -> std::io::Result<()> {
    let source = fs::read_to_string(path)?;
    let file = syn::parse_file(&source)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
    let mut visitor = CallVisitor {
        path,
        calls: HashSet::new(),
    };
    visitor.visit_file(&file);

    if !visitor.calls.is_empty() {
        calls.push(CallVisitorItem {
            calls: visitor.calls,
            file_name: path
                .file_name()
                .map_or_else(|| "[unknown]".to_string(), |n| n.to_string_lossy().into_owned()),
        });
    }
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

            if args.len() != 2 {
                fail("expected two arguments: `tail_call!(&ctx, \"name\")`");
            }

            match args.get(1) {
                Some(Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                })) => self.calls.insert(s.value()),
                _ => fail("second argument must be a string literal"),
            };
        }

        syn::visit::visit_macro(self, m);
    }
}
