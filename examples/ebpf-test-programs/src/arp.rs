use core::{mem, net::Ipv4Addr};

use aya_ebpf::{macros::map, maps::Array, programs::TcContext};
use etherparse::{
    ArpHardwareId, ArpOperation, ArpPacket, ArpPacketSlice, EtherType, Ethernet2Header,
    Ethernet2HeaderSlice, PacketBuilder,
};
use gladia_ebpf::{
    TestStatus, TestSuite, act, arrange, assert, assert_test, test_log, util::ref_at,
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EndpointConfig {
    pub mac: [u8; 6],
    pub ipv4: Ipv4Addr,
}

#[map(name = "endpoint_config")]
static ENDPOINT_CONFIG: Array<EndpointConfig> = Array::pinned(1, 0);

const SRC_MAC: [u8; 6] = [0xde, 0xad, 0xbe, 0xef, 0xde, 0xef];
const DST_MAC: [u8; 6] = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
const SRC_IP: [u8; 4] = [110, 0, 11, 1];
const DST_IP: [u8; 4] = [172, 16, 10, 1];

const ENDPOINT_MAC: [u8; 6] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
const ENDPOINT_IP: [u8; 4] = [192, 168, 20, 7];

#[arrange(tc, "l2_announcement_arp_no_entry")]
pub fn l2_announcement_arp_no_entry_arrange(ctx: TcContext) -> TestStatus {
    let config = EndpointConfig {
        mac: ENDPOINT_MAC,
        ipv4: Ipv4Addr::from_octets(ENDPOINT_IP),
    };
    let _ = ENDPOINT_CONFIG.set(0, &config, 0);
    build_packet(ctx)
}

#[act(tc, "l2_announcement_arp_no_entry")]
pub fn l2_announcement_arp_no_entry_act(ctx: TcContext) -> TestStatus {
    tail_call!(&ctx, "from_netdev")
}

#[assert(tc, "l2_announcement_arp_no_entry")]
pub fn l2_announcement_arp_no_entry_assert(ctx: TcContext, t: &mut TestSuite) {
    let data = ctx.data();
    let data_end = ctx.data_end();

    let len = data_end - data;
    assert_test!(t, len == 42, "ctx too short: need %d bytes", 42);

    test_log!(t, "length=%llu", len as u64);

    let Ok(bytes) = (unsafe { ref_at::<[u8; 42]>(&ctx, 0) }) else {
        return;
    };

    let Ok(eth) = Ethernet2HeaderSlice::from_slice(bytes) else {
        assert_test!(t, false, "failed to parse ethernet header");
        return;
    };

    assert_test!(
        t,
        eth.ether_type() == EtherType::ARP,
        "expected ARP ether header"
    );

    assert_test!(t, eth.destination() == SRC_MAC, "destination mac");
    assert_test!(t, eth.source() == ENDPOINT_MAC, "source mac");

    let Ok(arp) = ArpPacketSlice::from_slice(&bytes[Ethernet2Header::LEN..]) else {
        assert_test!(t, false, "failed to parse ARP packet");
        return;
    };

    assert_test!(
        t,
        arp.operation() == ArpOperation::REPLY,
        "ARP reply operation"
    );

    assert_test!(
        t,
        arp.hw_addr_size() == 6 && arp.proto_addr_size() == 4,
        "unexpected ARP address sizes"
    );

    assert_test!(t, arp.sender_hw_addr() == ENDPOINT_MAC, "ARP sha");
    assert_test!(t, arp.sender_protocol_addr() == DST_IP, "ARP spa");

    assert_test!(t, arp.target_hw_addr() == SRC_MAC, "ARP sha");
    assert_test!(t, arp.target_protocol_addr() == SRC_IP, "ARP tpa");

    let len = mem::size_of::<u32>();
    test_log!(t, "expected drop, got forward, flags=%llu", len as u64);
}

#[inline]
fn build_packet(ctx: TcContext) -> TestStatus {
    let arp = ArpPacket::new(
        ArpHardwareId::ETHERNET,
        EtherType::IPV4,
        ArpOperation::REQUEST,
        &SRC_MAC,
        &SRC_IP,
        &DST_MAC,
        &DST_IP,
    )
    .unwrap();

    let builder = PacketBuilder::ethernet2(SRC_MAC, DST_MAC).arp(arp);

    let mut packet = [0u8; 42];

    builder.write_to_slice(&mut packet).unwrap();

    let mut builder = gladia_ebpf::util::PacketBuilder::new(&ctx);
    builder.push_data(&packet);
    builder.build();
    TestStatus::Pass
}
