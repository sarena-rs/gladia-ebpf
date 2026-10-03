#![allow(nonstandard_style, dead_code)]

use aya_ebpf::programs::TcContext;
use gladia_ebpf::TestStatus;

pub fn container_receive_packet(ctx: TcContext) -> TestStatus {
    let _ = tail_call!(&ctx, "from_container");

    TestStatus::FrameworkError
}

pub fn container_send_packet(ctx: TcContext) -> TestStatus {
    let _ = tail_call!(&ctx, "to_container");

    TestStatus::FrameworkError
}

pub fn host_receive_packet(ctx: TcContext) -> TestStatus {
    let _ = tail_call!(&ctx, "to_host");

    TestStatus::FrameworkError
}

pub fn host_send_packet(ctx: TcContext) -> TestStatus {
    let _ = tail_call!(&ctx, "from_host");

    TestStatus::FrameworkError
}

pub fn netdev_receive_packet(ctx: TcContext) -> TestStatus {
    let _ = tail_call!(&ctx, "from_netdev");

    TestStatus::FrameworkError
}

pub fn netdev_send_packet(ctx: TcContext) -> TestStatus {
    let _ = tail_call!(&ctx, "to_netdev");

    TestStatus::FrameworkError
}

pub fn overlay_receive_packet(ctx: TcContext) -> TestStatus {
    let _ = tail_call!(&ctx, "from_overlay");

    TestStatus::FrameworkError
}

pub fn overlay_send_packet(ctx: TcContext) -> TestStatus {
    let _ = tail_call!(&ctx, "to_overlay");

    TestStatus::FrameworkError
}

pub fn wireguard_receive_packet(ctx: TcContext) -> TestStatus {
    let _ = tail_call!(&ctx, "from_wireguard");

    TestStatus::FrameworkError
}

pub fn wireguard_send_packet(ctx: TcContext) -> TestStatus {
    let _ = tail_call!(&ctx, "to_wireguard");

    TestStatus::FrameworkError
}
