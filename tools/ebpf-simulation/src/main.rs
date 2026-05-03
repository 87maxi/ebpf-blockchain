use clap::Parser;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::Rng;
use reqwest::Client;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::StreamExt;
use uuid::Uuid;
use chrono::Utc;

// ============================================================================
// CLI Configuration
// ============================================================================

/// eBPF Blockchain Traffic Simulator
///
/// Generates realistic blockchain traffic with configurable load patterns,
/// transaction types, and attack simulation capabilities.
#[derive(Parser, Debug, Clone)]
#[command(name = "ebpf-simulation")]
#[command(about = "Realistic eBPF Blockchain traffic simulator")]
#[command(version)]
struct Cli {
    /// Comma-separated list of target node IPs
    #[arg(long, default_value = "192.168.2.13")]
    nodes: String,

    /// Duration in seconds (0 = infinite)
    #[arg(long, default_value = "0")]
    duration: u64,

    /// Load pattern: realistic, constant, attack
    #[arg(long, default_value = "realistic")]
    load_pattern: String,

    /// Rate of malicious transactions (0.0 - 1.0)
    #[arg(long, default_value = "0.1")]
    malicious_rate: f64,

    /// RPC port
    #[arg(long, default_value = "9090")]
    rpc_port: u16,

    /// WebSocket port
    #[arg(long, default_value = "9090")]
    ws_port: u16,
}

// ============================================================================
// Transaction Types
// ============================================================================

/// Represents the different types of transactions the simulator can generate
#[derive(Debug, Clone)]
enum TxType {
    /// Normal token transfer
    Transfer {
        from: String,
        to: String,
        amount: f64,
    },
    /// Smart contract call
    ContractCall {
        contract: String,
        method: String,
        params: Vec<String>,
    },
    /// Stake/Unstake operation
    Stake {
        validator: String,
        amount: f64,
        action: String,
    },
    /// Governance vote
    Governance {
        proposal: String,
        vote: String,
        weight: f64,
    },
    /// Replay attack - reuses old nonce
    ReplayAttack {
        #[allow(dead_code)] original_id: String,
        #[allow(dead_code)] original_nonce: u64,
    },
    /// Double spend attempt
    DoubleSpend {
        #[allow(dead_code)] original_tx: String,
        #[allow(dead_code)] conflicting_tx: String,
        #[allow(dead_code)] amount: f64,
    },
    /// Sybil node transaction
    SybilTx {
        #[allow(dead_code)] fake_identity: String,
        #[allow(dead_code)] stake: f64,
    },
    /// DDoS flood transaction
    DDoSTx {
        #[allow(dead_code)] target: String,
        #[allow(dead_code)] payload_size: usize,
    },
}

/// Complete transaction structure sent to the node
#[derive(Debug, Clone, Serialize)]
struct Transaction {
    id: String,
    data: String,
    #[serde(default)]
    nonce: u64,
    #[serde(default)]
    timestamp: u64,
    #[serde(skip)]
    tx_type: TxType,
    #[serde(skip)]
    sender: String,
}

impl Transaction {
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_else(|_| 0)
    }
}

// ============================================================================
// Load Pattern Configuration
// ============================================================================

/// Load phases in a 60-second cycle
#[derive(Debug, Clone)]
enum LoadPhase {
    Low,    // 0-15s: 5s intervals
    Medium, // 15-30s: 2s intervals
    High,   // 30-45s: 0.5s intervals
    Peak,   // 45-60s: 0.1s intervals
}

impl LoadPhase {
    fn from_cycle_time(seconds_in_cycle: u64) -> Self {
        match seconds_in_cycle {
            0..=15 => LoadPhase::Low,
            16..=30 => LoadPhase::Medium,
            31..=45 => LoadPhase::High,
            _ => LoadPhase::Peak,
        }
    }

    fn base_interval(&self) -> Duration {
        match self {
            LoadPhase::Low => Duration::from_secs(5),
            LoadPhase::Medium => Duration::from_secs(2),
            LoadPhase::High => Duration::from_millis(500),
            LoadPhase::Peak => Duration::from_millis(100),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            LoadPhase::Low => "🟢 LOW",
            LoadPhase::Medium => "🟡 MEDIUM",
            LoadPhase::High => "🟠 HIGH",
            LoadPhase::Peak => "🔴 PEAK",
        }
    }
}

// ============================================================================
// Simulator Engine
// ============================================================================

struct SimulatorEngine {
    cli: Cli,
    rng: StdRng,
    nonces: HashMap<String, u64>,
    sent_transactions: Vec<String>,
    old_nonces: HashMap<String, u64>,
    tx_counter: u64,
    malicious_counter: u64,
    start_time: u64,
}

impl SimulatorEngine {
    fn new(cli: Cli) -> Self {
        let seed = u64::from_le_bytes(*b"ebpf_sim");
        Self {
            cli,
            rng: StdRng::seed_from_u64(seed),
            nonces: HashMap::new(),
            sent_transactions: Vec::new(),
            old_nonces: HashMap::new(),
            tx_counter: 0,
            malicious_counter: 0,
            start_time: Transaction::current_timestamp(),
        }
    }

    fn get_next_nonce(&mut self, sender: &str) -> u64 {
        let current = self.nonces.entry(sender.to_string()).or_insert(0);
        let nonce = *current;
        *current += 1;
        nonce
    }

    fn generate_sender_id(&mut self) -> String {
        let prefixes = ["alice", "bob", "charlie", "dave", "eve", "frank", "grace", "heidi"];
        let prefix = prefixes[self.rng.gen_range(0..prefixes.len())];
        format!("{}-{}", prefix, self.rng.gen_range(1..100))
    }

    fn generate_normal_transaction(&mut self) -> Transaction {
        let sender = self.generate_sender_id();
        let nonce = self.get_next_nonce(&sender);
        let tx_type = self.choose_normal_tx_type();
        
        let (id, data) = match &tx_type {
            TxType::Transfer { from, to, amount } => {
                let id = Uuid::new_v4().to_string()[..8].to_string();
                let data = serde_json::json!({
                    "action": "transfer",
                    "from": from,
                    "to": to,
                    "amount": amount,
                    "timestamp": Transaction::current_timestamp()
                }).to_string();
                (id, data)
            }
            TxType::ContractCall { contract, method, params } => {
                let id = Uuid::new_v4().to_string()[..8].to_string();
                let data = serde_json::json!({
                    "action": "contract_call",
                    "contract": contract,
                    "method": method,
                    "params": params,
                    "timestamp": Transaction::current_timestamp()
                }).to_string();
                (id, data)
            }
            TxType::Stake { validator, amount, action } => {
                let id = Uuid::new_v4().to_string()[..8].to_string();
                let data = serde_json::json!({
                    "action": action,
                    "validator": validator,
                    "amount": amount,
                    "timestamp": Transaction::current_timestamp()
                }).to_string();
                (id, data)
            }
            TxType::Governance { proposal, vote, weight } => {
                let id = Uuid::new_v4().to_string()[..8].to_string();
                let data = serde_json::json!({
                    "action": "governance_vote",
                    "proposal": proposal,
                    "vote": vote,
                    "weight": weight,
                    "timestamp": Transaction::current_timestamp()
                }).to_string();
                (id, data)
            }
            _ => {
                let id = Uuid::new_v4().to_string()[..8].to_string();
                let data = serde_json::json!({
                    "action": "unknown",
                    "timestamp": Transaction::current_timestamp()
                }).to_string();
                (id, data)
            }
        };

        Transaction {
            id,
            data,
            nonce,
            timestamp: Transaction::current_timestamp(),
            tx_type,
            sender,
        }
    }

    fn choose_normal_tx_type(&mut self) -> TxType {
        let roll = self.rng.gen_range(0.0..1.0);
        let amount = self.rng.gen_range(0.001..1000.0);
        
        match roll {
            0.0..=0.4 => {
                let from = self.generate_sender_id();
                let to = self.generate_sender_id();
                TxType::Transfer { from, to, amount }
            }
            0.4..=0.65 => {
                let contracts = ["StakingPool", "Governance", "TokenBridge", "NFTMarket", "LiquidityPool"];
                let methods = ["execute", "approve", "transfer", "mint", "burn"];
                TxType::ContractCall {
                    contract: contracts[self.rng.gen_range(0..contracts.len())].to_string(),
                    method: methods[self.rng.gen_range(0..methods.len())].to_string(),
                    params: vec![format!("param_{}", self.rng.gen_range(1..100))],
                }
            }
            0.65..=0.8 => {
                let validators = ["validator-1", "validator-2", "validator-3", "validator-4"];
                let actions = ["stake", "unstake"];
                TxType::Stake {
                    validator: validators[self.rng.gen_range(0..validators.len())].to_string(),
                    amount,
                    action: actions[self.rng.gen_range(0..actions.len())].to_string(),
                }
            }
            _ => {
                let votes = ["for", "against", "abstain"];
                TxType::Governance {
                    proposal: format!("PROP-{}", self.rng.gen_range(1..50)),
                    vote: votes[self.rng.gen_range(0..votes.len())].to_string(),
                    weight: self.rng.gen_range(1.0..100.0),
                }
            }
        }
    }

    fn generate_malicious_transaction(&mut self) -> Transaction {
        let roll = self.rng.gen_range(0.0..1.0);
        let sender = self.generate_sender_id();
        let nonce = self.get_next_nonce(&sender);
        
        let (id, data, tx_type) = match roll {
            0.0..=0.3 => {
                let old_nonce = self.old_nonces.get(&sender)
                    .copied()
                    .unwrap_or_else(|| self.rng.gen_range(0..10));
                let uuid_str = Uuid::new_v4().to_string();
                let original_id = format!("replay-{}", &uuid_str[..8]);
                let data = serde_json::json!({
                    "action": "transfer",
                    "from": sender,
                    "to": "attacker-wallet",
                    "amount": 500.0,
                    "timestamp": Transaction::current_timestamp(),
                    "note": "REPLAY_ATTACK_SIMULATION"
                }).to_string();
                let tx = TxType::ReplayAttack {
                    original_id: original_id.clone(),
                    original_nonce: old_nonce,
                };
                (original_id, data, tx)
            }
            0.3..=0.55 => {
                let uuid1 = Uuid::new_v4().to_string();
                let uuid2 = Uuid::new_v4().to_string();
                let original_tx = format!("tx-{}", &uuid1[..8]);
                let conflicting_tx = format!("tx-{}", &uuid2[..8]);
                let amount = self.rng.gen_range(100.0..1000.0);
                let data = serde_json::json!({
                    "action": "double_spend",
                    "original_tx": original_tx.clone(),
                    "conflicting_tx": conflicting_tx.clone(),
                    "amount": amount,
                    "timestamp": Transaction::current_timestamp(),
                    "note": "DOUBLE_SPEND_SIMULATION"
                }).to_string();
                let tx = TxType::DoubleSpend {
                    original_tx,
                    conflicting_tx,
                    amount,
                };
                let uuid3 = Uuid::new_v4().to_string();
                (format!("doublespend-{}", &uuid3[..8]), data, tx)
            }
            0.55..=0.8 => {
                let fake_identity = format!("sybil-node-{}", self.rng.gen_range(1000..9999));
                let stake = self.rng.gen_range(0.001..1.0);
                let data = serde_json::json!({
                    "action": "register_validator",
                    "identity": fake_identity.clone(),
                    "stake": stake,
                    "timestamp": Transaction::current_timestamp(),
                    "note": "SYBIL_ATTACK_SIMULATION"
                }).to_string();
                let tx = TxType::SybilTx {
                    fake_identity: fake_identity.clone(),
                    stake,
                };
                let uuid_str = Uuid::new_v4().to_string();
                (format!("sybil-{}", &uuid_str[..8]), data, tx)
            }
            _ => {
                let target = format!("node-{}", self.rng.gen_range(1..10));
                let payload_size = self.rng.gen_range(100..10000);
                let payload = "A".repeat(payload_size);
                let data = serde_json::json!({
                    "action": "flood",
                    "target": target.clone(),
                    "payload_size": payload_size,
                    "payload": payload,
                    "timestamp": Transaction::current_timestamp(),
                    "note": "DDOS_ATTACK_SIMULATION"
                }).to_string();
                let tx = TxType::DDoSTx {
                    target: target.clone(),
                    payload_size,
                };
                let uuid_str = Uuid::new_v4().to_string();
                (format!("ddos-{}", &uuid_str[..8]), data, tx)
            }
        };

        self.old_nonces.insert(sender.clone(), nonce);

        Transaction {
            id,
            data,
            nonce,
            timestamp: Transaction::current_timestamp(),
            tx_type,
            sender,
        }
    }

    fn generate_transaction(&mut self, is_malicious: bool) -> Transaction {
        if is_malicious {
            self.malicious_counter += 1;
            self.generate_malicious_transaction()
        } else {
            self.generate_normal_transaction()
        }
    }

    fn should_generate_malicious(&mut self) -> bool {
        let rate = self.cli.malicious_rate.min(1.0).max(0.0);
        self.rng.gen_range(0.0..1.0) < rate
    }

    fn get_interval_for_pattern(&mut self, cycle_time: u64) -> Duration {
        match self.cli.load_pattern.as_str() {
            "constant" => Duration::from_secs(2),
            "attack" => {
                let phase = LoadPhase::from_cycle_time(cycle_time);
                let base = phase.base_interval();
                self.add_jitter(base, 0.1)
            }
            _ => {
                let phase = LoadPhase::from_cycle_time(cycle_time);
                let base = phase.base_interval();
                self.add_jitter(base, 0.2)
            }
        }
    }

    fn add_jitter(&mut self, base: Duration, jitter_ratio: f64) -> Duration {
        let jitter_ms = (base.as_millis() as f64) * jitter_ratio;
        let jitter = self.rng.gen_range(-jitter_ms..jitter_ms);
        let new_ms = (base.as_millis() as f64 + jitter).max(10.0) as u64;
        Duration::from_millis(new_ms)
    }

    fn tx_type_label(&self, tx: &Transaction) -> &'static str {
        match tx.tx_type {
            TxType::Transfer { .. } => "💸 Transfer",
            TxType::ContractCall { .. } => "📜 Contract",
            TxType::Stake { .. } => "🔒 Stake",
            TxType::Governance { .. } => "🗳️ Governance",
            TxType::ReplayAttack { .. } => "🔄 Replay⚠️",
            TxType::DoubleSpend { .. } => "💰 DoubleSpend⚠️",
            TxType::SybilTx { .. } => "👥 Sybil⚠️",
            TxType::DDoSTx { .. } => "💣 DDoS⚠️",
        }
    }
}

// ============================================================================
// WebSocket Monitor
// ============================================================================

async fn spawn_ws_monitor(node_ip: &str, ws_port: u16) {
    let ws_url = format!("ws://{}:{}/ws", node_ip, ws_port);
    
    tokio::spawn(async move {
        println!("⏳ Connecting to WebSocket: {}", ws_url);
        match connect_async(&ws_url).await {
            Ok((mut ws_stream, _)) => {
                println!("✅ WebSocket Connected! Monitoring events...");
                while let Some(msg) = ws_stream.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                match json["event"].as_str() {
                                    Some("BlockCreated") => {
                                        let hash_val = json["hash"].as_str().unwrap_or("?");
                                        let hash_short = &hash_val[..hash_val.len().min(10)];
                                        println!("📦 BLOCK CREATED: #{} | Hash: {}... | Proposer: {} | Tx Count: {}",
                                            json["height"].as_u64().unwrap_or(0),
                                            hash_short,
                                            json["proposer"].as_str().unwrap_or("?"),
                                            json["tx_count"].as_u64().unwrap_or(0)
                                        );
                                    }
                                    Some("BlockConfirmed") => {
                                        println!("✅ BLOCK CONFIRMED: #{} | Voters: {}",
                                            json["height"].as_u64().unwrap_or(0),
                                            json["voters"].as_u64().unwrap_or(0)
                                        );
                                    }
                                    Some("BlockRejected") => {
                                        println!("❌ BLOCK REJECTED: #{} | Reason: {}",
                                            json["height"].as_u64().unwrap_or(0),
                                            json["reason"].as_str().unwrap_or("unknown")
                                        );
                                    }
                                    Some("SecurityAlert") => {
                                        println!("🚨 SECURITY ALERT: Type={} | Source={} | Action={}",
                                            json["type"].as_str().unwrap_or("?"),
                                            json["source"].as_str().unwrap_or("?"),
                                            json["action"].as_str().unwrap_or("?")
                                        );
                                    }
                                    Some("TxProcessed") => {
                                        println!("📝 TX PROCESSED: {} | Status: {}",
                                            json["tx_id"].as_str().unwrap_or("?"),
                                            json["status"].as_str().unwrap_or("?")
                                        );
                                    }
                                    Some(other) => {
                                        let text_short: String = text.chars().take(100).collect();
                                        println!("📩 Event: {} | Data: {}", other, text_short);
                                    }
                                    None => {
                                        let text_short: String = text.chars().take(100).collect();
                                        println!("📩 Raw WS Message: {}", text_short);
                                    }
                                }
                            } else {
                                let text_short: String = text.chars().take(100).collect();
                                println!("📩 Raw: {}", text_short);
                            }
                        }
                        Ok(_) => (),
                        Err(e) => {
                            eprintln!("❌ WS Error: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to connect to WebSocket {}: {}", ws_url, e);
                eprintln!("   Make sure the node is running with WebSocket enabled");
            }
        }
    });
}

// ============================================================================
// Main Simulation Loop
// ============================================================================

async fn run_simulation(engine: Arc<Mutex<SimulatorEngine>>) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    
    let nodes_str = {
        let eng = engine.lock().await;
        eng.cli.nodes.clone()
    };
    let nodes: Vec<&str> = nodes_str.split(',').collect();
    let duration = {
        let eng = engine.lock().await;
        eng.cli.duration
    };
    let rpc_port = {
        let eng = engine.lock().await;
        eng.cli.rpc_port
    };
    
    let duration_label = if duration == 0 { "∞" } else { &duration.to_string() };
    
    println!("\n🎯 Starting Simulation");
    println!("   Nodes: {}", nodes.join(", "));
    println!("   Pattern: {}", { let eng = engine.lock().await; eng.cli.load_pattern.clone() });
    println!("   Malicious Rate: {:.0}%", { let eng = engine.lock().await; eng.cli.malicious_rate * 100.0 });
    println!("   Duration: {}s\n", duration_label);
    
    let mut cycle_counter: u64 = 0;
    let mut last_phase: String = String::new();
    let start_instant = std::time::Instant::now();
    
    loop {
        let (interval, tx, tx_id, tx_type_label, sender, nonce, is_malicious) = {
            let mut eng = engine.lock().await;
            
            // Check duration limit
            if duration > 0 {
                let elapsed = Transaction::current_timestamp().saturating_sub(eng.start_time);
                if elapsed >= duration {
                    println!("\n⏱️  Duration limit reached ({}s). Stopping simulation.", duration);
                    break;
                }
            }
            
            let cycle_time = cycle_counter % 60;
            let phase = LoadPhase::from_cycle_time(cycle_time);
            let phase_label = phase.label().to_string();
            
            if phase_label != last_phase {
                println!("\n--- Load Phase: {} (Cycle: {}s/60s) ---", phase_label, cycle_time);
                last_phase = phase_label;
            }
            
            let interval = eng.get_interval_for_pattern(cycle_time);
            let is_malicious = eng.should_generate_malicious();
            let tx = eng.generate_transaction(is_malicious);
            
            let tx_id = tx.id.clone();
            let tx_type_label = eng.tx_type_label(&tx).to_string();
            let sender = tx.sender.clone();
            let nonce = tx.nonce;
            
            eng.tx_counter += 1;
            
            (interval, tx, tx_id, tx_type_label, sender, nonce, is_malicious)
        };
        
        let primary_node = nodes[cycle_counter as usize % nodes.len()];
        let rpc_url = format!("http://{}:{}/api/v1/transactions", primary_node, rpc_port);
        
        let timestamp = Utc::now().format("%H:%M:%S%.3f");
        
        match client.post(&rpc_url).json(&tx).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    let status_icon = if is_malicious { "🔴" } else { "🟢" };
                    println!("  [{}] {} Sent: {} | Type: {} | Sender: {} | Nonce: {} | Status: {}",
                        timestamp, status_icon, tx_id, tx_type_label, sender, nonce, resp.status());
                } else {
                    let status_icon = if is_malicious { "🟡" } else { "⚠️" };
                    println!("  [{}] {} Failed: {} | Type: {} | Status: {}",
                        timestamp, status_icon, tx_id, tx_type_label, resp.status());
                }
            }
            Err(e) => {
                eprintln!("  [{}] ❌ Error sending {}: {}", timestamp, tx_id, e);
            }
        }
        
        {
            let mut eng = engine.lock().await;
            eng.sent_transactions.push(tx_id);
            if eng.sent_transactions.len() > 100 {
                eng.sent_transactions.remove(0);
            }
        }
        
        cycle_counter += 1;
        sleep(interval).await;
    }
    
    let (total, malicious, elapsed) = {
        let eng = engine.lock().await;
        (eng.tx_counter, eng.malicious_counter, start_instant.elapsed().as_secs_f64())
    };
    
    println!("\n📊 Simulation Statistics:");
    println!("   Total Transactions: {}", total);
    println!("   Malicious Transactions: {}", malicious);
    println!("   Legitimate Transactions: {}", total - malicious);
    println!("   Duration: {:.1}s", elapsed);
    
    Ok(())
}

// ============================================================================
// Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║     eBPF Blockchain Traffic Simulator v0.2.0          ║");
    println!("╚════════════════════════════════════════════════════════╝");
    
    println!("📍 Target Nodes: {}", cli.nodes);
    println!("📡 RPC Port: {}", cli.rpc_port);
    println!("🔗 WS Port: {}", cli.ws_port);
    
    let node_list: Vec<&str> = cli.nodes.split(',').collect();
    for node_ip in &node_list {
        spawn_ws_monitor(node_ip.trim(), cli.ws_port).await;
    }
    
    sleep(Duration::from_millis(500)).await;
    
    let engine = Arc::new(Mutex::new(SimulatorEngine::new(cli)));
    run_simulation(engine).await?;
    
    println!("\n👋 Simulation complete. Goodbye!");
    Ok(())
}
