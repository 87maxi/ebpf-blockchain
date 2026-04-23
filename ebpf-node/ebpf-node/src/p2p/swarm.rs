use std::time::Duration;

use libp2p::{
    gossipsub,
    identify,
    kad,
    mdns,
    noise,
    request_response,
    Swarm,
    SwarmBuilder,
    tcp,
    yamux,
    Multiaddr,
    PeerId,
};

use crate::config::node::{SyncRequest, SyncResponse};
use crate::p2p::behaviour::MyBehaviour;

/// GossipSub topic with namespace for versioning and network isolation.
/// Format: /<application>/<protocol>/<version>
/// This prevents cross-network message confusion and enables protocol versioning.
pub const GOSSIPSUB_TOPIC: &str = "/ebpf-blockchain/consensus/1.0.0";

/// Create gossipsub behaviour with the given keypair
pub fn create_gossipsub(
    keypair: libp2p::identity::Keypair,
) -> anyhow::Result<gossipsub::Behaviour> {
    let message_id_fn = |message: &gossipsub::Message| {
        let mut s = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&message.data, &mut s);
        gossipsub::MessageId::from(std::hash::Hasher::finish(&s).to_string())
    };

    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(1))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .build()
        .map_err(|msg| anyhow::anyhow!("gossipsub config error: {}", msg))?;

    let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(keypair.clone()),
        gossipsub_config,
    ).map_err(|msg| anyhow::anyhow!("gossipsub behaviour error: {}", msg))?;

    Ok(gossipsub)
}

/// Create the P2P swarm with all behaviours
/// TAREA 2.5: Includes Kademlia DHT configuration
/// CHANGE 6: Enhanced Kademlia configuration for server mode
pub fn create_swarm(
    keypair: libp2p::identity::Keypair,
    gossipsub: gossipsub::Behaviour,
) -> anyhow::Result<Swarm<MyBehaviour>> {
    let peer_id = keypair.public().to_peer_id();
    
    let identify = identify::Behaviour::new(identify::Config::new(
        "/ebpf-blockchain/1.0.0".into(),
        keypair.public(),
    ));

    let mdns = mdns::tokio::Behaviour::new(
        mdns::Config::default(),
        peer_id,
    )?;

    let sync = request_response::cbor::Behaviour::new(
        [(libp2p::StreamProtocol::new(SyncRequest::protocol()), request_response::ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    // TAREA 2.5: Configure Kademlia DHT
    let kad_store = kad::store::MemoryStore::new(peer_id);
    let protocol_id = libp2p::StreamProtocol::new("/ebpf-blockchain/kad/1.0.0");
    
    // CHANGE 6: Configure Kademlia with proper parameters for server mode
    let kad_config = {
        let mut config = kad::Config::new(protocol_id);
        config.set_publication_interval(Some(Duration::from_secs(60)));
        config.set_provider_publication_interval(Some(Duration::from_secs(3600)));
        config
    };
    
    let kademlia = kad::Behaviour::with_config(peer_id, kad_store, kad_config);

    let swarm = SwarmBuilder::with_existing_identity(keypair.clone().into())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|_key| {
            Ok(MyBehaviour {
                gossipsub,
                identify,
                kademlia,
                sync,
                mdns,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(120)))
        .build();

    Ok(swarm)
}

/// Setup listen addresses and subscribe to gossip topic
pub fn setup_listening_and_subscription(
    swarm: &mut Swarm<MyBehaviour>,
    listen_addrs: Vec<Multiaddr>,
) -> anyhow::Result<libp2p::gossipsub::IdentTopic> {
    for addr in &listen_addrs {
        tracing::info!("Listening on: {}", addr);
        swarm.listen_on(addr.clone())?;
    }

    // Subscribe to Gossip topic using namespaced topic
    let topic = gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC);
    let _ = swarm.behaviour_mut().gossipsub.subscribe(&topic);

    Ok(topic)
}
