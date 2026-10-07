use aya_ebpf::{
    EbpfContext as _, bindings::__sk_buff, helpers::generated::bpf_redirect_peer,
    programs::TcContext,
};
use aya_log_ebpf::info;
use network_types::{arp::ArpHdr, eth::EthHdr};

use crate::{
    error::{Res, Verdict},
    netdev::EndpointConfig,
    ref_at::{ref_at, ref_at_mut},
};

// ARP protocol opcodes.
pub const ARPOP_REQUEST: u16 = 1; // ARP request			
pub const ARPOP_REPLY: u16 = 2; // ARP reply			

pub const ARPHRD_ETHER: u16 = 1; //  Ethernet 10Mbps		

pub const ETH_BROADCAST: [u8; 6] = [0xff; 6];

#[inline(always)]
pub fn process_arp(ctx: &TcContext, config: &EndpointConfig) -> Res<Verdict> {
    let eth: &EthHdr = unsafe { ref_at(ctx, 0)? };
    let arp: &ArpHdr = unsafe { ref_at(ctx, EthHdr::LEN)? };

    let is_request = arp.oper() == ARPOP_REQUEST && arp.htype() == ARPHRD_ETHER;
    let dst_mac = eth.dst_addr;
    let src_mac = eth.src_addr;
    let sender_ip = arp.spa();
    let target_ip = arp.tpa();

    info!(ctx, "arp: dst mac {:mac}, src mac {:mac}", dst_mac, src_mac);

    let for_us =
        eth_is_bcast(&dst_mac) || bpf_memcmp(dst_mac.as_ptr(), config.mac.as_ptr(), 6) == 0;
    if !is_request || !for_us {
        return Ok(Verdict::Pass);
    }

    // Our own endpoint IP is answered by the stack, not here.
    if target_ip == config.ipv4.octets() {
        return Ok(Verdict::Next);
    }

    info!(
        ctx,
        "arp: who-has {:i}? replying with {:mac}", target_ip, config.mac
    );

    write_arp_reply(ctx, config.mac, src_mac, target_ip, sender_ip)?;

    let ifindex = ctx_get_ifindex(ctx);
    Ok(Verdict::Redirect(ctx_redirect_peer(ifindex, 0) as i32))
}

#[inline]
fn eth_is_bcast(a: &[u8; 6]) -> bool {
    bpf_memcmp(a.as_ptr(), ETH_BROADCAST.as_ptr(), ETH_BROADCAST.len()) == 0
}

/// Turn the ARP request currently in the packet into a reply from us.
#[inline(always)]
fn write_arp_reply(
    ctx: &TcContext,
    our_mac: [u8; 6],
    requester_mac: [u8; 6],
    our_ip: [u8; 4],
    requester_ip: [u8; 4],
) -> Res<()> {
    let eth: &mut EthHdr = unsafe { ref_at_mut(ctx, 0)? };
    eth.src_addr = our_mac;
    eth.dst_addr = requester_mac;

    let arp: &mut ArpHdr = unsafe { ref_at_mut(ctx, EthHdr::LEN)? };
    arp.set_oper(ARPOP_REPLY);
    arp.set_sha(our_mac);
    arp.set_spa(our_ip);
    arp.set_tha(requester_mac);
    arp.set_tpa(requester_ip);

    Ok(())
}

pub fn ctx_redirect_peer(ifindex: u32, flags: u64) -> i64 {
    unsafe { bpf_redirect_peer(ifindex, flags) }
}

pub fn ctx_get_ifindex(ctx: &TcContext) -> u32 {
    let skb = ctx.as_ptr() as *const __sk_buff;
    let ifindex = unsafe { (*skb).ifindex };
    ifindex
}

#[inline(always)]
pub fn bpf_memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    unsafe {
        for i in 0..n {
            let a = core::ptr::read(s1.wrapping_add(i));
            let b = core::ptr::read(s2.wrapping_add(i));
            if a != b {
                return a as i32 - b as i32;
            }
        }
    }
    0
}
