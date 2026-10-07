use core::net::Ipv4Addr;

use aya_ebpf::{macros::map, maps::Array, programs::TcContext};
use aya_log_ebpf::info;
use network_types::eth::{EthHdr, EtherType};

use crate::{
    EbpfError,
    arp::process_arp,
    error::{Res, Verdict},
    ref_at::ref_at,
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EndpointConfig {
    pub mac: [u8; 6],
    pub ipv4: Ipv4Addr,
}

#[map(name = "endpoint_config")]
static ENDPOINT_CONFIG: Array<EndpointConfig> = Array::pinned(1, 0);

#[inline(always)]
pub fn try_from_netdev(ctx: TcContext) -> Res<Verdict> {
    let eth: &EthHdr = unsafe { ref_at(&ctx, 0)? };

    let Ok(ether_type) = eth.ether_type() else {
        return Ok(Verdict::Pass);
    };

    let config = get_endpoint_config()?;
    info!(
        &ctx,
        "endpoint config: {:mac}, ip: {:i}", config.mac, config.ipv4
    );

    if ether_type == EtherType::Arp {
        return process_arp(&ctx, config);
    }
    Ok(Verdict::Drop)
}

#[inline(always)]
pub fn try_to_netdev(_ctx: TcContext) -> Res<Verdict> {
    Ok(Verdict::Pass)
}

#[inline(always)]
pub fn get_endpoint_config<'a>() -> Res<&'a EndpointConfig> {
    ENDPOINT_CONFIG.get(0).ok_or(EbpfError::InternalError)
}
