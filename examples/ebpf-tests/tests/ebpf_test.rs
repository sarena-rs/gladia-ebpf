// Built by `just build-ebpf`; `EBPF_OBJECTS` is set in `.cargo/config.toml`.
static PROGRAMS: &[u8] =
    aya::include_bytes_aligned!(concat!(env!("EBPF_OBJECTS"), "/ebpf-programs.o"));
static TEST_PROGRAMS: &[u8] =
    aya::include_bytes_aligned!(concat!(env!("EBPF_OBJECTS"), "/ebpf-test-programs.o"));

#[test]
#[ignore = "requires CAP_NET_ADMIN/CAP_SYS_ADMIN and a writable /run/netns"]
fn ebpf_test_runner() -> gladia::Res<()> {
    gladia::run_ebpf_test(PROGRAMS, TEST_PROGRAMS)
}
