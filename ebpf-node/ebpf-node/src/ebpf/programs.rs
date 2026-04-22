// ebpf/programs.rs
use aya::{
    programs::{KProbe, Xdp, XdpFlags},
    Ebpf,
};
use tracing::info;

/// Attach the XDP program to the given interface
pub fn attach_xdp(ebpf: &mut Ebpf, iface: &str) -> anyhow::Result<()> {
    let program = ebpf.program_mut("ebpf_node").ok_or_else(|| anyhow::anyhow!("ebpf_node program not found"))?;
    let mut xdp: &mut Xdp = program.try_into()?;
    xdp.load()?;
    xdp.attach(iface, XdpFlags::default())?;
    info!("XDP program attached to {}", iface);
    Ok(())
}

/// Attach the KProbe for inbound traffic
pub fn attach_kprobe_in(ebpf: &mut Ebpf) -> anyhow::Result<()> {
    let program = ebpf.program_mut("netif_receive_skb").ok_or_else(|| anyhow::anyhow!("netif_receive_skb program not found"))?;
    let mut kprobe: &mut KProbe = program.try_into()?;
    kprobe.load()?;
    kprobe.attach("netif_receive_skb", 0)?;
    info!("KProbe in attached to netif_receive_skb");
    Ok(())
}

/// Attach the KProbe for outbound traffic
pub fn attach_kprobe_out(ebpf: &mut Ebpf) -> anyhow::Result<()> {
    let program = ebpf.program_mut("napi_consume_skb").ok_or_else(|| anyhow::anyhow!("napi_consume_skb program not found"))?;
    let mut kprobe: &mut KProbe = program.try_into()?;
    kprobe.load()?;
    kprobe.attach("napi_consume_skb", 0)?;
    info!("KProbe out attached to napi_consume_skb");
    Ok(())
}

/// Attach all eBPF programs
pub fn attach_all(ebpf: &mut Ebpf, iface: &str) -> anyhow::Result<()> {
    attach_xdp(ebpf, iface)?;
    attach_kprobe_in(ebpf)?;
    attach_kprobe_out(ebpf)?;
    Ok(())
}

/// Detach all eBPF programs (for hot-reload)
/// In aya 0.13.1, detach() requires link_ids that are managed internally.
/// We simply drop the programs which will automatically detach on Drop.
pub fn detach_all(_ebpf: &mut Ebpf) {
    // In aya 0.13.1, the Xdp and KProbe types require a link_id to detach().
    // The link_ids are managed internally by ProgramData and not accessible.
    // When the Ebpf object is dropped, all programs and their links are automatically detached.
    // For hot-reload, we need to reload a fresh Ebpf instance instead.
    info!("Hot-reload: eBPF programs will be detached when Ebpf instance is dropped");
}
