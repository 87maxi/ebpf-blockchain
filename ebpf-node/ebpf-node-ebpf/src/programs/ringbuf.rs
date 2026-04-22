//! Ringbuf implementation for eBPF programs
//! This module provides a way to efficiently send data from eBPF programs to user-space
//! using the modern Ringbuf mechanism instead of bpf_trace_printk

use aya_ebpf::{
    maps::RingBuf,
    programs::ProbeContext,
};

// Define the structure for latency events
#[repr(C)]
pub struct LatencyEvent {
    pub source_ip: u32,
    pub latency: u64,
}

// Define the structure for packet events
#[repr(C)]
pub struct PacketEvent {
    pub source_ip: u32,
    pub packet_size: u32,
}

// Ringbuf for latency events
pub static LATENCY_RINGBUF: RingBuf = RingBuf::with_byte_size(1024 * 1024, 0);

// Ringbuf for packet events
pub static PACKET_RINGBUF: RingBuf = RingBuf::with_byte_size(1024 * 1024, 0);