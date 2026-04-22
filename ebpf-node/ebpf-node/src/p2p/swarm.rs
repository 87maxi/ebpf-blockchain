use std::time::Duration;

use libp2p::{
    gossipsub,
    identify,
    mdns,
    noise,
    request_response,
    Swarm,
    SwarmBuilder,
    tcp,
    yamux,
    Multiaddr,
};

use crate::config::node::{SyncRequest, SyncResponse};
use crate::p2p::behaviour::MyBehaviour;

/// Create a gossipsub behaviour with the given keypair
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
pub fn create_swarm(
    keypair: libp2p::identity::Keypair,
    gossipsub: gossipsub::Behaviour,
) -> anyhow::Result<Swarm<MyBehaviour>> {
    let identify = identify::Behaviour::new(identify::Config::new(
        "/ebpf-blockchain/1.0.0".into(),
        keypair.public(),
    ));

    let mdns = mdns::tokio::Behaviour::new(
        mdns::Config::default(),
        keypair.public().to_peer_id(),
    )?;

    let sync = request_response::cbor::Behaviour::new(
        [(libp2p::StreamProtocol::new(SyncRequest::protocol()), request_response::ProtocolSupport::Full)],
        request_response::Config::default(),
    );

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

    // Subscribe to Gossip topic
    let topic = gossipsub::IdentTopic::new("gossip");
    let _ = swarm.behaviour_mut().gossipsub.subscribe(&topic);

    Ok(topic)
}
