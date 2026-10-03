use std::{env, fmt::Write, fs, path::PathBuf};

use gladia_shared::STRING_SIZE;

use crate::{
    collect::visit_dir,
    constants::{TAIL_CALL_MACRO_NAME, TAIL_CALL_MAP_NAME},
};

pub fn build_mapping() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = manifest.join("src");

    // The unique calls in sorted order, so the name -> index assignment is stable across builds.
    // A call's index is its position in this list.
    let calls: Vec<String> = visit_dir(&src)
        .unwrap_or_else(|e| panic!("failed to process source tree: {e}"))
        .into_iter()
        .collect();

    // Without calls there is no map or table; a program array with 0 entries would fail to load.
    // The macro is always generated, so an unknown call gets a clear error from its catch-all arm.
    let mut contents = String::new();
    if !calls.is_empty() {
        build_call_map(&calls, &mut contents);
        build_call_table(&calls, &mut contents);
    }
    build_tail_call_macro(&calls, &mut contents);

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("__gladia_tail_calls.rs");
    fs::write(out, contents).unwrap();
}

fn build_call_map(calls: &[String], contents: &mut String) {
    writeln!(
        contents,
        "#[aya_ebpf::macros::map(name = \"{TAIL_CALL_MAP_NAME}\")]\n\
         #[doc(hidden)]\n\
         #[allow(non_upper_case_globals)]\n\
         pub static {TAIL_CALL_MAP_NAME}: aya_ebpf::maps::ProgramArray = \
         aya_ebpf::maps::ProgramArray::with_max_entries({}, 0);\n",
        calls.len()
    )
    .unwrap();
}

fn build_call_table(calls: &[String], contents: &mut String) {
    let count = calls.len();
    writeln!(
        contents,
        "#[used]\n\
         #[doc(hidden)]\n\
         #[allow(non_upper_case_globals)]\n\
         #[unsafe(link_section = \".gladia_tail_call_section\")]\n\
         pub static __gladia_TAIL_CALL_MAP: gladia_ebpf::TestEntryHeader<{count}> = \
         gladia_ebpf::TestEntryHeader {{\n    \
             version: 1u32,\n    \
             count: {count}u32,\n    \
             size: core::mem::size_of::<gladia_ebpf::TestEntryCall>() as u32,\n    \
             entries: ["
    )
    .unwrap();
    for (index, call) in calls.iter().enumerate() {
        writeln!(
            contents,
            "        gladia_ebpf::TestEntryCall {{ index: {index}u32, name: {} }},",
            name_literal(call)
        )
        .unwrap();
    }
    writeln!(contents, "    ],\n}};\n").unwrap();
}

fn build_tail_call_macro(calls: &[String], contents: &mut String) {
    // Exported so a binary can use the macro from the library that includes this file.
    // A successful tail call never returns, so the macro only evaluates to its value when the
    // call fails.
    writeln!(
        contents,
        "#[allow(unused_macros)]\n#[macro_export]\nmacro_rules! {TAIL_CALL_MACRO_NAME} {{"
    )
    .unwrap();
    for (index, call) in calls.iter().enumerate() {
        writeln!(
            contents,
            "    ($ctx:expr, {call:?}) => {{{{ \
             let _ = unsafe {{ $crate::{TAIL_CALL_MAP_NAME}.tail_call($ctx, {index}u32) }}; \
             gladia_ebpf::TestStatus::FrameworkError \
             }}}};"
        )
        .unwrap();
    }
    // Cargo reruns the build script whenever a source file changes, so every call that is used has
    // an arm above. This arm only matches where that output is stale, as in an editor that has not
    // rerun the build script yet. It still evaluates to a `TestStatus` to avoid a type error there.
    writeln!(
        contents,
        "    ($ctx:expr, $name:literal) => {{{{ \
         compile_error!(concat!(\"unknown tail call `\", $name, \"`; rebuild so the build script picks it up\")); \
         gladia_ebpf::TestStatus::FrameworkError \
         }}}};"
    )
    .unwrap();
    writeln!(contents, "}}").unwrap();
}

/// Formats `name` as a byte string literal padded with zeros to `STRING_SIZE` bytes. The
/// literal is dereferenced (`*b"..."`) so it has type `[u8; STRING_SIZE]`.
fn name_literal(name: &str) -> String {
    let bytes = name.as_bytes();
    assert!(
        bytes.len() <= STRING_SIZE,
        "name `{name}` is longer than {STRING_SIZE} bytes"
    );

    let mut literal = String::from("*b\"");
    literal.extend(
        bytes
            .iter()
            .copied()
            .flat_map(std::ascii::escape_default)
            .map(char::from),
    );
    literal.push_str(&"\\0".repeat(STRING_SIZE - bytes.len()));
    literal.push('"');
    literal
}
