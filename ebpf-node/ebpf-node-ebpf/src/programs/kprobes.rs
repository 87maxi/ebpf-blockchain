//! KProbe programs implementation

use aya_ebpf::{
    helpers::bpf_ktime_get_ns,
    macros::kprobe,
    programs::ProbeContext,
};

// Import shared maps from the programs module
use crate::programs::maps::{LATENCY_STATS, START_TIMES};
// TODO: Re-enable Ringbuf once verifier issues are resolved
// use crate::programs::ringbuf::{LATENCY_RINGBUF, LatencyEvent};

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

    // Store start time in a map for later use (still needed for latency calculation)
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

        // TODO: Send latency data to user-space via Ringbuf (verifier issue)

        // Calculate power-of-2 bucket for the histogram (keep for backward compatibility)
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
