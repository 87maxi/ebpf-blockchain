use serde::{Deserialize, Serialize};
use tokio::time::{interval, Duration};
use reqwest::Client;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::StreamExt;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Transaction {
    pub id: String,
    pub data: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let node_ip = std::env::var("NODE_IP").unwrap_or_else(|_| "192.168.2.13".to_string());
    let rpc_url = format!("http://{}:9090/rpc", node_ip);
    let ws_url = format!("ws://{}:9090/ws", node_ip);

    println!("🚀 Starting eBPF Blockchain Simulator");
    println!("📍 Target Node: {}", node_ip);
    println!("📡 RPC URL: {}", rpc_url);
    println!("🔗 WS URL:  {}", ws_url);

    let client = Client::new();

    // Spawn WebSocket listener to see confirmations (Consensus)
    tokio::spawn(async move {
        println!("⏳ Connecting to WebSocket...");
        match connect_async(&ws_url).await {
            Ok((mut ws_stream, _)) => {
                println!("✅ WebSocket Connected! Waiting for network confirmations...");
                while let Some(msg) = ws_stream.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                    if json["event"] == "BlockConfirmed" {
                                        println!("🔔 CONSENSUS: Tx \"{}\" confirmed by Quorum! Voters: {}", 
                                            json["tx_id"], json["voters"]);
                                    } else {
                                        println!("📩 Other Event: {} | Data: {}", json["event"], text);
                                    }
                                } else {
                                    println!("📩 Raw WS Message: {}", text);
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
            Err(e) => eprintln!("❌ Failed to connect to WebSocket: {}", e),
        }
    });

    // Transaction generator loop
    let mut interval = interval(Duration::from_secs(3));
    println!("⚙️  Generator starting (Interval: 3s)...");
    
    loop {
        interval.tick().await;
        
        let tx = Transaction {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            data: format!("Fixture Tx generated at {:?}", std::time::SystemTime::now()),
        };

        match client.post(&rpc_url).json(&tx).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    println!("📤 Sent Tx: {} [Status: {}]", tx.id, resp.status());
                } else {
                    eprintln!("⚠️ Server returned error for Tx {}: {}", tx.id, resp.status());
                }
            }
            Err(e) => eprintln!("❌ Failed to send Tx {}: {}", tx.id, e),
        }
    }
}
