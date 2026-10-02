use std::{
    collections::{HashMap, HashSet},
    env,
    fmt::Write,
    fs,
    path::PathBuf,
};

use gladia_shared::STRING_SIZE;

use crate::{
    collect::{CallVisitorItem, visit_dir},
    constants::TAIL_CALL_MACRO_NAME,
};

/// The tail calls of one source file, each with its index into the program array.
struct IndexedCallItem {
    calls: Vec<IndexedCall>,
    file_name: String,
}

struct IndexedCall {
    index: u32,
    name: String,
}

pub fn build_mapping() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = manifest.join("src");

    let mut items: Vec<CallVisitorItem> = Vec::new();
    if let Err(e) = visit_dir(&src, &mut items) {
        panic!("failed to process source tree: {e}");
    }

    // Union the calls of every file into one set.
    let calls: HashSet<String> = items
        .iter()
        .flat_map(|item| item.calls.iter().cloned())
        .collect();

    // Sort so the name -> index assignment is stable across builds.
    let mut calls: Vec<String> = calls.into_iter().collect();
    calls.sort();

    // Give every unique call the index of its position in the sorted list.
    let indices: HashMap<&str, u32> = calls
        .iter()
        .enumerate()
        .map(|(index, call)| (call.as_str(), index as u32))
        .collect();

    // Rebuild the per-file items, now with each call's index.
    let indexed_items: Vec<IndexedCallItem> = items
        .iter()
        .map(|item| {
            let mut calls: Vec<IndexedCall> = item
                .calls
                .iter()
                .map(|call| IndexedCall {
                    index: indices[call.as_str()],
                    name: call.clone(),
                })
                .collect();
            calls.sort_by_key(|call| call.index);
            IndexedCallItem {
                calls,
                file_name: item.file_name.clone(),
            }
        })
        .collect();

    let mut contents = String::new();
    build_call_map(&calls, &mut contents);
    build_call_table(&indexed_items, &mut contents);
    build_tail_call_macro(&calls, &mut contents);

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("__gladia_tail_calls.rs");
    fs::write(out, contents).unwrap();
}

fn build_call_map(calls: &[String], contents: &mut String) {
    writeln!(
        contents,
        "#[aya_ebpf::macros::map(name = \"__gladia_tail_call_map\")]\n"
    )
    .unwrap();
    writeln!(
        contents,
        "#[doc(hidden)]\n#[allow(non_upper_case_globals)]\npub static __gladia_tail_call_map: aya_ebpf::maps::ProgramArray = aya_ebpf::maps::ProgramArray::with_max_entries({}, 0);\n\n",
        calls.len()
    )
    .unwrap();
}

fn build_call_table(indexed_items: &[IndexedCallItem], contents: &mut String) {
    // One header per source file; the suffix keeps the static names unique.
    for (item_index, item) in indexed_items.iter().enumerate() {
        writeln!(contents, "#[used]").unwrap();
        writeln!(contents, "#[doc(hidden)]").unwrap();
        writeln!(contents, "#[allow(non_upper_case_globals)]").unwrap();
        writeln!(
            contents,
            "#[unsafe(link_section = \".gladia_tail_call_section\")]"
        )
        .unwrap();
        writeln!(
            contents,
            "pub static __gladia_TAIL_CALL_MAP_{item_index}: gladia_ebpf::TestEntryHeader<{}> = gladia_ebpf::TestEntryHeader {{",
            item.calls.len()
        )
        .unwrap();
        writeln!(contents, "    version: 1u32,").unwrap();
        writeln!(
            contents,
            "    file_name: {},",
            name_literal(&item.file_name)
        )
        .unwrap();
        writeln!(contents, "    count: {}u32,", item.calls.len()).unwrap();
        writeln!(
            contents,
            "    size: core::mem::size_of::<gladia_ebpf::TestEntryCall>() as u32,"
        )
        .unwrap();
        writeln!(contents, "    entries: [").unwrap();
        for call in &item.calls {
            writeln!(
                contents,
                "        gladia_ebpf::TestEntryCall {{ index: {}u32, name: {} }},",
                call.index,
                name_literal(&call.name)
            )
            .unwrap();
        }
        writeln!(contents, "    ],").unwrap();
        writeln!(contents, "}};\n").unwrap();
    }
}

fn build_tail_call_macro(calls: &[String], contents: &mut String) {
    if !calls.is_empty() {
        // Exported so a binary can use the macro from the library that includes this file.
        writeln!(contents, "#[allow(unused_macros)]").unwrap();
        writeln!(contents, "#[macro_export]").unwrap();
        writeln!(contents, "macro_rules! {TAIL_CALL_MACRO_NAME} {{").unwrap();
        for (index, call) in calls.iter().enumerate() {
            writeln!(
            contents,
            "    ($ctx:expr, {call:?}) => {{ unsafe {{ $crate::__gladia_tail_call_map.tail_call($ctx, {index}u32) }} }};"
        )
        .unwrap();
        }
        writeln!(contents, "}}\n").unwrap();
    }
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
    for &byte in bytes {
        literal.extend(std::ascii::escape_default(byte).map(char::from));
    }
    literal.push_str(&"\\0".repeat(STRING_SIZE - bytes.len()));
    literal.push('"');
    literal
}
