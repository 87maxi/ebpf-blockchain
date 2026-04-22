//! Hot-reload architecture for eBPF programs
//! This module provides functionality to dynamically reload eBPF programs
//! without restarting the entire node.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn};

use aya::{Ebpf, programs::{KProbe, Xdp, XdpFlags}};
use aya::programs::Program;

use crate::ebpf::loader::load_binary;
use crate::ebpf::programs::{attach_all, detach_all};

/// Hot-reload manager for eBPF programs
pub struct EbpfHotReloadManager {
    /// Current eBPF instance
    ebpf: Arc<Mutex<Option<Ebpf>>>,
    /// Interface name for XDP program
    iface: String,
}

impl EbpfHotReloadManager {
    /// Create a new hot-reload manager
    pub fn new(iface: String) -> Self {
        Self {
            ebpf: Arc::new(Mutex::new(None)),
            iface,
        }
    }

    /// Initialize the hot-reload manager with the current eBPF instance
    /// Note: Programs are already attached by load() in main.rs,
    /// so we only load the binary without attaching.
    pub async fn init(&self) -> Result<(), anyhow::Error> {
        let mut ebpf_guard = self.ebpf.lock().await;
        let loaded_ebpf = load_binary()?;
        *ebpf_guard = Some(loaded_ebpf);
        
        // Programs are already attached by load() in main.rs
        // Do NOT attach again to avoid "Device or resource busy" error
        info!("eBPF hot-reload manager initialized (programs already attached by load())");
        Ok(())
    }

    /// Reload eBPF programs from scratch
    pub async fn reload(&self) -> Result<(), anyhow::Error> {
        info!("Initiating eBPF program reload...");
        
        // Detach all existing programs
        if let Some(ref mut ebpf) = *self.ebpf.lock().await {
            detach_all(ebpf);
        }
        
        // Load new programs
        let loaded_ebpf = load_binary()?;
        
        {
            let mut ebpf_guard = self.ebpf.lock().await;
            *ebpf_guard = Some(loaded_ebpf);
            
            if let Some(ref mut ebpf) = *ebpf_guard {
                // Attach new programs
                attach_all(ebpf, &self.iface)?;
            }
        }
        
        info!("eBPF programs reloaded successfully");
        Ok(())
    }

    /// Get a reference to the current eBPF instance
    pub async fn get_ebpf(&self) -> Arc<Mutex<Option<Ebpf>>> {
        self.ebpf.clone()
    }
}
