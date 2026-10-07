static TEST_PROGRAMS: &[u8] =
    aya::include_bytes_aligned!(concat!(env!("EBPF_OBJECTS"), "/ebpf-selftest-programs.o"));

#[test]
#[ignore = "requires CAP_NET_ADMIN/CAP_SYS_ADMIN and a writable /run/netns"]
fn ebpf_selftest_runner() -> gladia::Res<()> {
    // The self-tests make no tail calls into production programs.
    gladia::run_ebpf_test(&[], TEST_PROGRAMS)
}
