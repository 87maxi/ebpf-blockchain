use aya::{Ebpf, EbpfLoader};
use aya::programs::{KProbe, Xdp, XdpFlags};
use tracing::{info, warn};

/// Load the eBPF program from the compiled binary
pub fn load(ebpf: &mut Ebpf, iface: &str) -> anyhow::Result<()> {
    // Attach XDP program
    let xdp_program: &mut Xdp = ebpf.program_mut("ebpf_node").unwrap().try_into()?;
    xdp_program.load()?;
    if let Err(e) = xdp_program.attach(iface, XdpFlags::default()) {
        warn!("Failed to attach XDP program, continuing: {}", e);
    } else {
        info!("XDP program attached to {}", iface);
    }

    // Attach KProbe for inbound traffic
    let kprobe_in: &mut KProbe = ebpf.program_mut("netif_receive_skb").unwrap().try_into()?;
    kprobe_in.load()?;
    if let Err(e) = kprobe_in.attach("netif_receive_skb", 0) {
        warn!("Failed to attach KProbe in, continuing: {}", e);
    } else {
        info!("KProbe in attached to netif_receive_skb");
    }

    // Attach KProbe for outbound traffic
    let kprobe_out: &mut KProbe = ebpf.program_mut("napi_consume_skb").unwrap().try_into()?;
    kprobe_out.load()?;
    if let Err(e) = kprobe_out.attach("napi_consume_skb", 0) {
        warn!("Failed to attach KProbe out, continuing: {}", e);
    } else {
        info!("KProbe out attached to napi_consume_skb");
    }

    Ok(())
}

/// Load the eBPF binary from the OUT_DIR
pub fn load_binary() -> anyhow::Result<Ebpf> {
    let ebpf = Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/ebpf-node"
    )))?;
    Ok(ebpf)
}
