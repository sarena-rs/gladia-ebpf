use std::{collections::HashSet, env, fmt::Write, fs, path::PathBuf};

use waggle_shared::{TestEntryCall, TestEntryHeader};

use crate::{
    collect::{CallVisitorItem, visit_dir},
    constants::TAIL_CALL_MACRO_NAME,
};

pub fn build_mapping() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = manifest.join("src");

    let mut calls: Vec<CallVisitorItem> = Vec::new();
    if let Err(e) = visit_dir(&src, &mut calls) {
        panic!("failed to process source tree: {e}");
    }

    let mut calls = HashSet::new();

    // Sort so the name -> index assignment is stable across builds.
    let mut calls: Vec<String> = calls.into_iter().collect();
    calls.sort();

    let mut contents = String::new();
    build_call_map(&calls, &mut contents);
    build_call_table(&calls, &mut contents);
    build_tail_call_macro(&calls, &mut contents);

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("__waggle_tail_calls.rs");
    fs::write(out, contents).unwrap();
}

fn build_call_map(calls: &[String], contents: &mut String) {
    writeln!(
        contents,
        "#[aya_ebpf::macros::map(name = \"__waggle_tail_call_map\")]\n"
    )
    .unwrap();
    writeln!(
        contents,
        "static __waggle_tail_call_map: aya_ebpf::maps::ProgramArray = aya_ebpf::maps::ProgramArray::with_max_entries({}, 0);\n\n",
        calls.len()
    )
    .unwrap();
}

fn build_call_table(calls: &[String], contents: &mut String) {
    writeln!(contents, "#[used]").unwrap();
    writeln!(contents, "#[doc(hidden)]").unwrap();
    writeln!(
        contents,
        "#[unsafe(link_section = \".waggle_tail_call_section\")]"
    )
    .unwrap();
    writeln!(
        contents,
        "pub static __WAGGLE_TAIL_CALL_MAP: waggle_ebpf::TestEntryHeader<{}> = waggle_ebpf::TestEntryHeader {{\n",
        calls.len()
    )
    .unwrap();
    writeln!(contents, "    version: 1u32,").unwrap();
    writeln!(contents, "    file_name: make_name(\"some.rs\"),").unwrap();
    writeln!(contents, "    count: 1u32,").unwrap();
    writeln!(
        contents,
        "    size: core::mem::size_of::<waggle_ebpf::TestEntryCall>() as u32,"
    )
    .unwrap();
    writeln!(contents, "    entries: [],").unwrap();

    writeln!(contents, "}};\n\n").unwrap();
}

fn build_tail_call_macro(calls: &[String], contents: &mut String) {
    writeln!(contents, "#[allow(unused_macros)]").unwrap();
    writeln!(contents, "macro_rules! {TAIL_CALL_MACRO_NAME} {{").unwrap();
    for (index, call) in calls.iter().enumerate() {
        writeln!(
            contents,
            "    ($ctx:expr, {call:?}) => {{ unsafe {{ __waggle_tail_call_map.tail_call($ctx, {index}u32) }} }};"
        )
        .unwrap();
    }
    writeln!(contents, "}}\n").unwrap();
}

// #[used]
// #[unsafe(link_section = ".test_entry_calls")]
// static __WAGGLE_TAIL_CALL_MAP: TestEntryHeader<3> = TestEntryHeader {
//     version: 1,
//     file_name: make_name(b"yyy.rs"),
//     count: 3u32,
//     size: core::mem::size_of::<TestEntryCall>() as u32,
//     entries: [
//         TestEntryCall {
//             index: 0u32,
//             name: make_name(b"aaa"),
//         },
//         TestEntryCall {
//             index: 1u32,
//             name: make_name(b"bbb"),
//         },
//         TestEntryCall {
//             index: 2u32,
//             name: make_name(b"ccc"),
//         },
//     ],
// };

const fn make_name<const N: usize>(name: &[u8]) -> [u8; N] {
    let mut result = [0u8; N];

    // TODO: panic if name.len() > N
    let mut i = 0;
    while i < name.len() && i < N {
        result[i] = name[i];
        i += 1;
    }

    result
}
