//! XDP program implementation

use aya_ebpf::{
    bindings::xdp_action,
    macros::xdp,
    maps::lpm_trie::Key,
    programs::{XdpContext},
};
use core::mem;
use network_types::{
    eth::{EthHdr, EtherType},
    ip::Ipv4Hdr,
};

// Import shared maps from the programs module
use crate::programs::maps::{NODES_WHITELIST, NODES_BLACKLIST};
// TODO: Re-enable Ringbuf once verifier issues are resolved
// use crate::programs::ringbuf::{PACKET_RINGBUF, PacketEvent};

#[xdp]
pub fn ebpf_node(ctx: XdpContext) -> u32 {
    match try_ebpf_node(ctx) {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

#[inline(always)]
unsafe fn ptr_at<T>(ctx: &XdpContext, offset: usize) -> Result<*const T, ()> {
    let start = ctx.data();
    let end = ctx.data_end();
    let len = mem::size_of::<T>();

    if start + offset + len > end {
        return Err(());
    }

    Ok((start + offset) as *const T)
}

/// Convert [u8; 4] in network byte order (big-endian) to host u32
#[inline(always)]
fn bytes_to_u32(b: [u8; 4]) -> u32 {
    u32::from_be_bytes(b)
}

fn try_ebpf_node(ctx: XdpContext) -> Result<u32, ()> {
    let ethhdr: *const EthHdr = unsafe { ptr_at(&ctx, 0)? };
    let ether_type = unsafe { (*ethhdr).ether_type };
    match ether_type {
        0x0800 => {} // IPv4
        _ => return Ok(xdp_action::XDP_PASS),
    }

    let ipv4hdr: *const Ipv4Hdr = unsafe { ptr_at(&ctx, mem::size_of::<EthHdr>())? };
    let source_addr_bytes: [u8; 4] = unsafe { (*ipv4hdr).src_addr };
    // Convert from network byte order (big-endian) to host byte order
    let source_addr = bytes_to_u32(source_addr_bytes);

    // SECURITY: Whitelist XDP - Preventive filtering
    // First check if the source IP is in the blacklist (reactive - detected malicious)
    let blacklist_key = Key::<u32>::new(32, source_addr);
    if NODES_BLACKLIST.get(&blacklist_key).is_some() {
        // TODO: Send packet event to user-space via Ringbuf (verifier issue)
        return Ok(xdp_action::XDP_DROP);
    }
    
    // Then check if the source IP is in the whitelist (preventive - trusted)
    // If NOT in whitelist, drop the packet
    let whitelist_key = Key::<u32>::new(32, source_addr);
    if NODES_WHITELIST.get(&whitelist_key).is_none() {
        // IP not in whitelist - drop packet
        // TODO: Send packet event to user-space via Ringbuf (verifier issue)
        return Ok(xdp_action::XDP_DROP);
    }

    // IP is in whitelist and not in blacklist - allow
    // TODO: Send packet event to user-space via Ringbuf (verifier issue)
    Ok(xdp_action::XDP_PASS)
}
