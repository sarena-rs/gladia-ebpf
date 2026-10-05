use std::{
    collections::BTreeMap,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use aya::{
    Ebpf, EbpfLoader, Pod, TestRun, TestRunOptions,
    maps::{Array, MapData, ProgramArray},
    programs::SchedClassifier,
};
use gladia_shared::{ScapyAssert, TEST_RESULT_MAP_SIZE, TestStatus, tlv_reader};

use crate::{
    Res, TestRunnerError, constants::TAIL_CALL_MAP_NAME, reader::find_entry_calls, report,
};

const PAGE_SIZE: usize = 4096;
const CTX_SIZE: usize = 256;
const HEADROOM: usize = 256;
const TAILROOM: usize = 320;

/// Prefix the `arrange`/`act`/`assert` macros give the programs they generate:
/// `__test_fw_<kind>_<test name>`.
const TEST_PROGRAM_PREFIX: &str = "__test_fw_";

#[derive(Default)]
struct ProgramSet {
    arrange: Option<String>,
    act: Option<String>,
    assert: Option<String>,
}

impl ProgramSet {
    fn names(&self) -> impl Iterator<Item = &str> {
        [&self.arrange, &self.act, &self.assert]
            .into_iter()
            .flatten()
            .map(String::as_str)
    }
}

/// The maps through which the test programs report back.
struct TestMaps {
    result: Array<MapData, [u8; TEST_RESULT_MAP_SIZE]>,
    status_code: Array<MapData, u32>,
    scapy_assert: Array<MapData, ScapyAssert>,
    scapy_assert_count: Array<MapData, u32>,
}

impl TestMaps {
    fn take(bpf: &mut Ebpf) -> Res<Self> {
        Ok(Self {
            result: take_array(bpf, "test_suite_result")?,
            status_code: take_array(bpf, "test_suite_status_code")?,
            scapy_assert: take_array(bpf, "scapy_assert_map")?,
            scapy_assert_count: take_array(bpf, "scapy_assert_map_count")?,
        })
    }
}

/// Where the maps of the loaded programs are pinned, unless another directory is given.
pub const DEFAULT_PIN_DIR: &str = "/sys/fs/bpf/gladia";

/// Runs every test in the `test_programs` eBPF object against the production `programs` object,
/// pinning maps under [`DEFAULT_PIN_DIR`].
///
/// Both are the bytes of compiled ELF objects. How they are built is up to the caller (aya-build,
/// a custom xtask, ...). Embed them with `aya::include_bytes_aligned!` so they can be parsed in
/// place.
pub fn run_ebpf_test(programs: &[u8], test_programs: &[u8]) -> Res<()> {
    run_ebpf_test_with_pin_dir(DEFAULT_PIN_DIR, programs, test_programs)
}

/// Like [`run_ebpf_test`], but pins maps under `pin_dir`. The directory is emptied first.
pub fn run_ebpf_test_with_pin_dir(pin_dir: &str, programs: &[u8], test_programs: &[u8]) -> Res<()> {
    println!("\n");
    println!("\x1b[36m===== RUNNING eBPF TESTS =====\x1b[0m");
    println!("\n");

    reset_pin_dir(pin_dir)?;

    let entry_calls = find_entry_calls(test_programs)?;

    let mut prod_bpf = EbpfLoader::new()
        .default_map_pin_directory(format!("{pin_dir}/prod"))
        .load(programs)?;
    let mut test_bpf = EbpfLoader::new()
        .default_map_pin_directory(format!("{pin_dir}/test"))
        .load(test_programs)?;

    fill_entry_call_map(&mut prod_bpf, &mut test_bpf, &entry_calls)?;

    run_test(&mut test_bpf)
}

/// Loads every production program the test programs tail call into, and puts it in its slot of
/// the tail call map.
fn fill_entry_call_map(
    prod_bpf: &mut Ebpf,
    test_bpf: &mut Ebpf,
    entry_calls: &[(u32, String)],
) -> Res<()> {
    let map = test_bpf
        .map_mut(TAIL_CALL_MAP_NAME)
        .ok_or_else(|| TestRunnerError::MapNotFound(TAIL_CALL_MAP_NAME.to_owned()))?;
    let mut program_array = ProgramArray::try_from(map)?;

    for (slot, name) in entry_calls {
        load_bpf_program(prod_bpf, name)?;
        let fd = get_sched_classifier(prod_bpf, name)?.fd()?;
        program_array.set(*slot, fd, 0)?;
    }
    Ok(())
}

fn run_test(test_bpf: &mut Ebpf) -> Res<()> {
    // Group the test programs by test name.
    let mut groups: BTreeMap<String, ProgramSet> = BTreeMap::new();
    for (prog_name, _) in test_bpf.programs() {
        let Some((kind, test_name)) = prog_name
            .strip_prefix(TEST_PROGRAM_PREFIX)
            .and_then(|rest| rest.split_once('_'))
        else {
            continue;
        };

        let program_set = groups.entry(test_name.to_owned()).or_default();
        let slot = match kind {
            "arrange" => &mut program_set.arrange,
            "act" => &mut program_set.act,
            "assert" => &mut program_set.assert,
            _ => continue,
        };
        if slot.is_some() {
            return Err(TestRunnerError::DuplicateProgram {
                kind: kind.to_owned(),
                test: test_name.to_owned(),
            });
        }
        *slot = Some(prog_name.to_owned());
    }

    // Every test must have an assert program; load all of them.
    for (test_name, program_set) in &groups {
        if program_set.assert.is_none() {
            return Err(TestRunnerError::MissingCheck(test_name.clone()));
        }
        for name in program_set.names() {
            load_bpf_program(test_bpf, name)?;
        }
    }

    let mut maps = TestMaps::take(test_bpf)?;

    // Run every test, also after a failure, and report them all at the end.
    let mut summary = Summary::default();
    for (test_name, program_set) in &groups {
        let get = |name: &Option<String>| {
            name.as_deref()
                .map(|n| get_sched_classifier(test_bpf, n))
                .transpose()
        };
        let arrange_prog = get(&program_set.arrange)?;
        let act_prog = get(&program_set.act)?;
        // Checked above: every test has an assert program.
        let assert_prog = get(&program_set.assert)?.unwrap();

        let outcome = sub_test(test_name, &mut maps, arrange_prog, act_prog, assert_prog);
        if let Err(e) = &outcome {
            println!("\x1b[31m[ERROR]\x1b[0m {test_name}: {e}\n");
        }
        summary.record(test_name, outcome);
    }

    summary.print();
    summary.into_result()
}

/// The outcome of all tests in a run.
#[derive(Default)]
struct Summary {
    passed: usize,
    skipped: usize,
    /// The failed tests, each with the reason.
    failed: Vec<(String, String)>,
}

impl Summary {
    fn record(&mut self, test_name: &str, outcome: Res<TestStatus>) {
        let reason = match outcome {
            Ok(TestStatus::Pass) => {
                self.passed += 1;
                return;
            }
            Ok(TestStatus::Skip) => {
                self.skipped += 1;
                return;
            }
            Ok(TestStatus::Fail) => "test failed".to_owned(),
            Ok(TestStatus::FrameworkError) => "framework error".to_owned(),
            Err(e) => e.to_string(),
        };
        self.failed.push((test_name.to_owned(), reason));
    }

    fn total(&self) -> usize {
        self.passed + self.skipped + self.failed.len()
    }

    fn print(&self) {
        println!("\x1b[36m===== eBPF TEST SUMMARY =====\x1b[0m");
        println!(
            "{} tests: {} passed, {} failed, {} skipped",
            self.total(),
            self.passed,
            self.failed.len(),
            self.skipped
        );
        if !self.failed.is_empty() {
            println!("\nFailed:");
            for (test_name, reason) in &self.failed {
                println!("    \x1b[31m{test_name}\x1b[0m: {reason}");
            }
        }
        println!();
    }

    fn into_result(self) -> Res<()> {
        if self.failed.is_empty() {
            Ok(())
        } else {
            Err(TestRunnerError::TestsFailed {
                failed: self.failed.len(),
                total: self.total(),
            })
        }
    }
}

fn sub_test(
    name: &str,
    maps: &mut TestMaps,
    arrange_prog: Option<&SchedClassifier>,
    act_prog: Option<&SchedClassifier>,
    assert_prog: &SchedClassifier,
) -> Res<TestStatus> {
    let mut data = vec![0u8; PAGE_SIZE - HEADROOM - TAILROOM];
    let mut ctx = vec![0u8; CTX_SIZE];

    // Clear the results of the previous test.
    maps.result.set(0, &[0u8; TEST_RESULT_MAP_SIZE], 0)?;
    maps.scapy_assert_count.set(0, &0, 0)?;
    maps.status_code.set(0, &0, 0)?;

    if let Some(arrange_prog) = arrange_prog {
        let ret;
        (ret, data, ctx) = run_bpf_program(arrange_prog, &data, &ctx)?;
        if test_error(ret) {
            return Err(TestRunnerError::TestFailed(format!(
                "error while running arrange prog: status code ({ret})"
            )));
        }
    }

    if let Some(act_prog) = act_prog {
        let ret;
        (ret, data, ctx) = run_bpf_program(act_prog, &data, &ctx)?;
        if test_error(ret) {
            return Err(TestRunnerError::TestFailed(format!(
                "error while running act prog: status code ({ret})"
            )));
        }
        maps.status_code.set(0, &ret, 0)?;
    }

    run_bpf_program(assert_prog, &data, &ctx)?;

    let raw = maps.result.get(&0, 0)?;

    // Trim trailing zeroes — the eBPF side does not store a length
    // separately. A second map entry for the length would be cleaner
    // but this is sufficient for a fixed test buffer.
    let written = raw.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
    if written == 0 {
        return Err(TestRunnerError::NoResult);
    }

    let result = tlv_reader::parse_test(&raw[..written])?;

    report::print_test_result(&result);

    process_asserts(name, &maps.scapy_assert, &maps.scapy_assert_count)?;

    Ok(result.status)
}

fn take_array<V: Pod>(bpf: &mut Ebpf, name: &str) -> Res<Array<MapData, V>> {
    let map = bpf
        .take_map(name)
        .ok_or_else(|| TestRunnerError::MapNotFound(name.to_owned()))?;
    Ok(Array::try_from(map)?)
}

fn load_bpf_program(bpf: &mut Ebpf, name: &str) -> Res<()> {
    let program = bpf
        .program_mut(name)
        .ok_or_else(|| TestRunnerError::ProgramNotFound(name.to_string()))?;
    let program: &mut SchedClassifier = program.try_into()?;
    program.load()?;
    Ok(())
}

fn get_sched_classifier<'a>(bpf: &'a Ebpf, name: &str) -> Res<&'a SchedClassifier> {
    let sched = bpf
        .program(name)
        .ok_or_else(|| TestRunnerError::ProgramNotFound(name.to_string()))?
        .try_into()?;
    Ok(sched)
}

fn run_bpf_program(
    prog: &SchedClassifier,
    data: &[u8],
    ctx: &[u8],
) -> Res<(u32, Vec<u8>, Vec<u8>)> {
    let mut data_out = vec![0u8; data.len()];
    let mut ctx_out = vec![0u8; ctx.len()];

    let opts = TestRunOptions {
        data_in: Some(data),
        data_out: Some(&mut data_out),
        ctx_in: Some(ctx),
        ctx_out: Some(&mut ctx_out),
        repeat: 1,
        ..Default::default()
    };

    let ret = prog.test_run(opts)?;
    data_out.truncate(ret.data_size_out as usize);
    ctx_out.truncate(ret.ctx_size_out as usize);

    Ok((ret.return_value, data_out, ctx_out))
}

const SCAPY_TRACE_DIFF_CMD: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../scapy/trace_diff_pkts.py");
// The script's `#!/usr/bin/env python3` shebang needs the venv's python3 (with
// scapy installed) ahead of the system one on PATH — mirrors the justfile.
const SCAPYENV_BIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../scapyenv/bin");

fn process_asserts(
    name: &str,
    scapy_assert_map: &Array<MapData, ScapyAssert>,
    scapy_assert_map_count: &Array<MapData, u32>,
) -> Res<()> {
    let count = scapy_assert_map_count.get(&0, 0)?;
    if count == 0 {
        return Ok(());
    }

    let asserts: Vec<ScapyAssert> = (0..count)
        .map(|i| scapy_assert_map.get(&i, 0))
        .collect::<Result<_, _>>()?;
    let json_bytes = serde_json::to_vec(&asserts)?;

    println!("[{name}]  Scapy asserts failed: {count}");

    let path = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(PathBuf::from(SCAPYENV_BIN)).chain(std::env::split_paths(&path)),
    )
    .expect("scapyenv bin path and existing PATH must be joinable");

    let mut child = Command::new(SCAPY_TRACE_DIFF_CMD)
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .take()
        .expect("child stdin was piped")
        .write_all(&json_bytes)?;

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(TestRunnerError::TraceDiff(output.status));
    }

    println!(
        "\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

fn test_error(ret: u32) -> bool {
    ret == TestStatus::Fail as u32 || ret == TestStatus::FrameworkError as u32
}

fn reset_pin_dir(dir: &str) -> Res<()> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    std::fs::create_dir_all(format!("{dir}/prod"))?;
    std::fs::create_dir_all(format!("{dir}/test"))?;
    Ok(())
}
