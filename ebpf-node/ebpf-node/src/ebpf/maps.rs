// ebpf/maps.rs
use aya::{Ebpf, maps::{HashMap, LpmTrie, MapData}};
use aya::maps::lpm_trie::Key;
use anyhow::{Result, Context};

/// Type-safe eBPF map manager
pub struct EbpfMaps<'a> {
    ebpf: &'a mut Ebpf,
}

impl<'a> EbpfMaps<'a> {
    pub fn new(ebpf: &'a mut Ebpf) -> Self {
        Self { ebpf }
    }

    /// Type-safe access to LATENCY_STATS (HashMap<u64, u64>)
    pub fn latency_stats(&self) -> Result<HashMap<&MapData, u64, u64>> {
        let map = self.ebpf.map("LATENCY_STATS")
            .context("Failed to get LATENCY_STATS map")?;
        HashMap::try_from(map)
            .context("Failed to convert LATENCY_STATS to HashMap")
    }

    /// Type-safe access to NODES_WHITELIST (LpmTrie<u32, u32>)
    /// Read-only: uses &MapData (Borrow<MapData>)
    pub fn whitelist(&self) -> Result<LpmTrie<&MapData, u32, u32>> {
        let map = self.ebpf.map("NODES_WHITELIST")
            .context("Failed to get NODES_WHITELIST map")?;
        LpmTrie::try_from(map)
            .context("Failed to convert NODES_WHITELIST to LpmTrie")
    }

    /// Type-safe access to NODES_BLACKLIST (mutable LpmTrie)
    /// Mutable: uses &mut MapData (BorrowMut<MapData>) for insert/remove
    pub fn blacklist(&mut self) -> Result<LpmTrie<&mut MapData, u32, u32>> {
        let map = self.ebpf.map_mut("NODES_BLACKLIST")
            .context("Failed to get NODES_BLACKLIST map")?;
        LpmTrie::try_from(map)
            .context("Failed to convert NODES_BLACKLIST to LpmTrie")
    }

    /// Get whitelist size
    pub fn whitelist_size(&self) -> Result<usize> {
        let whitelist = self.whitelist()?;
        Ok(whitelist.iter().filter_map(|r| r.ok()).count())
    }

    /// Get blacklist size
    pub fn blacklist_size(&mut self) -> Result<usize> {
        let blacklist = self.blacklist()?;
        Ok(blacklist.iter().filter_map(|r| r.ok()).count())
    }

    /// Block IP in blacklist
    pub fn block_ip(&mut self, ip: u32, prefix_len: u32) -> Result<()> {
        let key = Key::new(prefix_len, ip);
        let mut blacklist = self.blacklist()?;
        blacklist.insert(&key, &1u32, 0)
            .context("Failed to insert IP into blacklist")
    }

    /// Unblock IP from blacklist
    pub fn unblock_ip(&mut self, ip: u32, prefix_len: u32) -> Result<()> {
        let key = Key::new(prefix_len, ip);
        let mut blacklist = self.blacklist()?;
        blacklist.remove(&key)
            .context("Failed to remove IP from blacklist")
    }

    /// Check if IP is in whitelist
    pub fn is_whitelisted(&self, ip: u32, prefix_len: u32) -> Result<bool> {
        let key = Key::new(prefix_len, ip);
        let whitelist = self.whitelist()?;
        Ok(whitelist.get(&key, 0).is_ok())
    }

    /// Get latency stats as a Vec<(bucket, count)>
    pub fn get_latency_stats(&self) -> Result<Vec<(u64, u64)>> {
        let stats = self.latency_stats()?;
        let mut result = Vec::new();
        for entry in stats.iter() {
            if let Ok((k, v)) = entry {
                result.push((k, v));
            }
        }
        Ok(result)
    }

    /// Get total packets processed (from XDP metrics)
    pub fn total_packets_processed(&self) -> i64 {
        let stats = match self.latency_stats() {
            Ok(s) => s,
            Err(_) => return 0,
        };
        let total: u64 = stats.iter()
            .filter_map(|entry| entry.ok().map(|(_, v)| v))
            .sum();
        total as i64
    }

    /// Get dropped packets count from XDP_DROP_COUNT map
    pub fn dropped_packets_count(&self) -> i64 {
        // Try to read from DROPPED_PACKETS map if it exists
        if let Some(map) = self.ebpf.map("DROPPED_PACKETS") {
            if let Ok(drop_map) = HashMap::<_, u32, u64>::try_from(map) {
                let total: u64 = drop_map.iter()
                    .filter_map(|entry| entry.ok().map(|(_, v)| v))
                    .sum();
                return total as i64;
            }
        }
        // Fallback: estimate from blacklist size as proxy for drops
        // In production, the eBPF program should maintain a drop counter
        0
    }
}
