#![no_std]
#![no_main]

use aya_ebpf::{
    bindings::{BPF_F_NO_PREALLOC, xdp_action},
    helpers::bpf_ktime_get_ns,
    macros::{kprobe, map, xdp},
    maps::HashMap,
    maps::lpm_trie::{Key, LpmTrie},
    programs::{ProbeContext, XdpContext},
};
use core::mem;
use network_types::{
    eth::{EthHdr, EtherType},
    ip::Ipv4Hdr,
};

#[map]
static NODES_BLACKLIST: LpmTrie<u32, u32> = LpmTrie::with_max_entries(1024, BPF_F_NO_PREALLOC);

/// Histogram of latencies.
/// Key is the bucket (power of 2 of the latency in nanoseconds), value is the count.
#[map]
static LATENCY_STATS: HashMap<u64, u64> = HashMap::with_max_entries(64, 0);

/// Temporary storage for packet start times, keyed by the skb pointer address.
#[map]
static START_TIMES: HashMap<u64, u64> = HashMap::with_max_entries(10240, 0);

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

fn try_ebpf_node(ctx: XdpContext) -> Result<u32, ()> {
    let ethhdr: *const EthHdr = unsafe { ptr_at(&ctx, 0)? };
    match unsafe { (*ethhdr).ether_type } {
        EtherType::Ipv4 => {}
        _ => return Ok(xdp_action::XDP_PASS),
    }

    let ipv4hdr: *const Ipv4Hdr = unsafe { ptr_at(&ctx, mem::size_of::<EthHdr>())? };
    let source_addr = unsafe { (*ipv4hdr).src_addr };

    // Check if the source IP is in the blacklist using longest prefix match.
    let key = Key::new(32, source_addr);
    if unsafe { NODES_BLACKLIST.get(&key) }.is_some() {
        return Ok(xdp_action::XDP_DROP);
    }

    Ok(xdp_action::XDP_PASS)
}

/// Kprobe attached to `netif_receive_skb` to record the entry time of a packet into the stack.
#[kprobe]
pub fn netif_receive_skb(ctx: ProbeContext) -> u32 {
    let _ = try_netif_receive_skb(ctx);
    0
}

fn try_netif_receive_skb(ctx: ProbeContext) -> Result<(), ()> {
    // The first argument to netif_receive_skb is a pointer to the sk_buff structure.
    let skb_ptr: u64 = ctx.arg(0).ok_or(())?;
    let start_time = unsafe { bpf_ktime_get_ns() };

    START_TIMES
        .insert(&skb_ptr, &start_time, 0)
        .map_err(|_| ())?;
    Ok(())
}

/// Kprobe attached to napi_consume_skb to calculate processing latency and update the histogram.
#[kprobe]
pub fn napi_consume_skb(ctx: ProbeContext) -> u32 {
    let _ = try_napi_consume_skb(ctx);
    0
}

fn try_napi_consume_skb(ctx: ProbeContext) -> Result<(), ()> {
    let skb_ptr: u64 = ctx.arg(0).ok_or(())?;

    if let Some(start_time) = unsafe { START_TIMES.get(&skb_ptr) } {
        let end_time = unsafe { bpf_ktime_get_ns() };
        let latency = end_time.saturating_sub(*start_time);

        // Calculate power-of-2 bucket for the histogram.
        let bucket = 64 - latency.leading_zeros() as u64;

        let count = unsafe { LATENCY_STATS.get(&bucket).copied().unwrap_or(0) };
        LATENCY_STATS
            .insert(&bucket, &(count + 1), 0)
            .map_err(|_| ())?;

        // Cleanup the start time entry.
        let _ = START_TIMES.remove(&skb_ptr);
    }

    Ok(())
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(link_section = "license")]
#[unsafe(no_mangle)]
static LICENSE: [u8; 13] = *b"Dual MIT/GPL\0";
