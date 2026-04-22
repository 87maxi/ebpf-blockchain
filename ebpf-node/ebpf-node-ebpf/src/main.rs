#![no_std]
#![no_main]

// Import the XDP program
mod programs;

// Re-export the XDP program
pub use programs::xdp::ebpf_node;

// Re-export the KProbe programs
pub use programs::kprobes::{netif_receive_skb, napi_consume_skb};

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(link_section = "license")]
#[unsafe(no_mangle)]
static LICENSE: [u8; 13] = *b"Dual MIT/GPL\0";
