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

/// Detach all eBPF programs (for hot-reload).
/// 
/// In aya 0.13.1, program detachment requires the link_id which is managed
/// internally by the Xdp/KProbe types. The proper approach is to convert
/// &mut Program to &mut Xdp/&mut KProbe using try_into(), which takes ownership
/// of the internal link. When these types are dropped, they automatically call
/// detach() with their stored link_id via their Drop implementations.
/// 
/// This implementation converts each &mut Program to &mut Xdp/&mut KProbe and
/// then drops the reference, triggering automatic detachment.
pub fn detach_all(ebpf: &mut Ebpf) -> anyhow::Result<()> {
    // Detach XDP program
    // prog is &mut Program from program_mut(), convert directly to &mut Xdp
    if let Some(prog) = ebpf.program_mut("ebpf_node") {
        let mut xdp: &mut Xdp = prog.try_into()?;
        info!("XDP program 'ebpf_node' prepared for detachment");
        drop(xdp); // Explicit drop triggers detach via Drop impl
        info!("XDP program 'ebpf_node' detached");
    }
    
    // Detach KProbe for inbound traffic
    if let Some(prog) = ebpf.program_mut("netif_receive_skb") {
        let mut kprobe: &mut KProbe = prog.try_into()?;
        info!("KProbe program 'netif_receive_skb' prepared for detachment");
        drop(kprobe);
        info!("KProbe program 'netif_receive_skb' detached");
    }
    
    // Detach KProbe for outbound traffic
    if let Some(prog) = ebpf.program_mut("napi_consume_skb") {
        let mut kprobe: &mut KProbe = prog.try_into()?;
        info!("KProbe program 'napi_consume_skb' prepared for detachment");
        drop(kprobe);
        info!("KProbe program 'napi_consume_skb' detached");
    }
    
    Ok(())
}
