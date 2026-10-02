use std::{collections::BTreeSet, env, fmt::Write, fs, path::PathBuf};

use gladia_shared::STRING_SIZE;

use crate::{
    collect::{CallVisitorItem, visit_dir},
    constants::TAIL_CALL_MACRO_NAME,
};

/// The tail calls of one source file, each with its index into the program array.
struct IndexedCallItem<'a> {
    calls: Vec<IndexedCall<'a>>,
    file_name: &'a str,
}

struct IndexedCall<'a> {
    index: u32,
    name: &'a str,
}

pub fn build_mapping() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = manifest.join("src");

    let items: Vec<CallVisitorItem> =
        visit_dir(&src).unwrap_or_else(|e| panic!("failed to process source tree: {e}"));

    // The unique calls in sorted order, so the name -> index assignment is stable across builds.
    let calls: Vec<&str> = items
        .iter()
        .flat_map(|item| item.calls.iter().map(String::as_str))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    // Rebuild the per-file items with each call's index: its position in the sorted list.
    let indexed_items: Vec<IndexedCallItem> = items
        .iter()
        .map(|item| {
            // Both lists are sorted by name, so these calls come out sorted by index as well.
            let calls = item
                .calls
                .iter()
                .map(|call| IndexedCall {
                    index: calls.binary_search(&call.as_str()).unwrap() as u32,
                    name: call,
                })
                .collect();
            IndexedCallItem {
                calls,
                file_name: &item.file_name,
            }
        })
        .collect();

    // Without calls there is nothing to generate; a program array with 0 entries would fail to
    // load.
    let mut contents = String::new();
    if !calls.is_empty() {
        build_call_map(&calls, &mut contents);
        build_call_table(&indexed_items, &mut contents);
        build_tail_call_macro(&calls, &mut contents);
    }

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("__gladia_tail_calls.rs");
    fs::write(out, contents).unwrap();
}

fn build_call_map(calls: &[&str], contents: &mut String) {
    writeln!(
        contents,
        "#[aya_ebpf::macros::map(name = \"__gladia_tail_call_map\")]\n\
         #[doc(hidden)]\n\
         #[allow(non_upper_case_globals)]\n\
         pub static __gladia_tail_call_map: aya_ebpf::maps::ProgramArray = \
         aya_ebpf::maps::ProgramArray::with_max_entries({}, 0);\n",
        calls.len()
    )
    .unwrap();
}

fn build_call_table(indexed_items: &[IndexedCallItem], contents: &mut String) {
    // One header per source file; the suffix keeps the static names unique.
    for (item_index, item) in indexed_items.iter().enumerate() {
        let count = item.calls.len();
        writeln!(
            contents,
            "#[used]\n\
             #[doc(hidden)]\n\
             #[allow(non_upper_case_globals)]\n\
             #[unsafe(link_section = \".gladia_tail_call_section\")]\n\
             pub static __gladia_TAIL_CALL_MAP_{item_index}: gladia_ebpf::TestEntryHeader<{count}> = \
             gladia_ebpf::TestEntryHeader {{\n    \
                 version: 1u32,\n    \
                 file_name: {},\n    \
                 count: {count}u32,\n    \
                 size: core::mem::size_of::<gladia_ebpf::TestEntryCall>() as u32,\n    \
                 entries: [",
            name_literal(item.file_name)
        )
        .unwrap();
        for call in &item.calls {
            writeln!(
                contents,
                "        gladia_ebpf::TestEntryCall {{ index: {}u32, name: {} }},",
                call.index,
                name_literal(call.name)
            )
            .unwrap();
        }
        writeln!(contents, "    ],\n}};\n").unwrap();
    }
}

fn build_tail_call_macro(calls: &[&str], contents: &mut String) {
    // Exported so a binary can use the macro from the library that includes this file.
    writeln!(
        contents,
        "#[allow(unused_macros)]\n#[macro_export]\nmacro_rules! {TAIL_CALL_MACRO_NAME} {{"
    )
    .unwrap();
    for (index, call) in calls.iter().enumerate() {
        writeln!(
            contents,
            "    ($ctx:expr, {call:?}) => {{ unsafe {{ $crate::__gladia_tail_call_map.tail_call($ctx, {index}u32) }} }};"
        )
        .unwrap();
    }
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
