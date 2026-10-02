use std::{collections::HashSet, env, fmt::Write, fs, path::PathBuf};

use crate::{collect::visit_dir, constants::TAIL_CALL_MACRO_NAME};

pub fn build_mapping() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = manifest.join("src");
    let mut calls = HashSet::new();

    if let Err(e) = visit_dir(&src, &mut calls) {
        panic!("failed to process source tree: {e}");
    }

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
        "#[aya_ebpf::macros::map(name = \"__waggle_entry_call_map\")]\n"
    )
    .unwrap();
    writeln!(
        contents,
        "static __waggle_entry_call_map: aya_ebpf::maps::ProgramArray = aya_ebpf::maps::ProgramArray::with_max_entries({}, 0);\n\n",
        calls.len()
    )
    .unwrap();
}

fn build_call_table(calls: &[String], contents: &mut String) {
    writeln!(contents, "#[doc(hidden)]").unwrap();
    writeln!(
        contents,
        "#[unsafe(link_section = \".waggle_entry_calls\")]"
    )
    .unwrap();
    writeln!(
        contents,
        "pub static __WAGGLE_CALL_MAP: &[(&str, u32)] = &[\n"
    )
    .unwrap();
    for (index, call) in calls.iter().enumerate() {
        writeln!(contents, "    ({call:?}, {index}u32),").unwrap();
    }
    writeln!(contents, "];\n\n").unwrap();
}

fn build_tail_call_macro(calls: &[String], contents: &mut String) {
    writeln!(contents, "#[allow(unused_macros)]").unwrap();
    writeln!(contents, "macro_rules! {TAIL_CALL_MACRO_NAME} {{").unwrap();
    for (index, call) in calls.iter().enumerate() {
        writeln!(
            contents,
            "    ($ctx:expr, {call:?}) => {{ unsafe {{ __waggle_entry_call_map.tail_call($ctx, {index}u32) }} }};"
        )
        .unwrap();
    }
    writeln!(contents, "}}\n").unwrap();
}

//
//
//

// let config = Config::new().scan_src().generate_mapping();
// config.run().unwrap();

// waggle::Builder::new()
//     .scan("src")
//     .generate("calls.rs")
//     .run()
//     .unwrap();

// Generate:
//
// pub mod __generated {
//   pub static CALLS: &[(&str, u32)] = &[
//     ("hello", 1"),
//     ("world", 2"),
// ];
// }

// #[used]
// #[unsafe(link_section = ".test_entry_calls")]
// static __TEST_ENTRY_CALLS: TestEntryHeader<3> = TestEntryHeader {
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
