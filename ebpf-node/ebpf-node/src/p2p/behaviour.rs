use libp2p::{gossipsub, identify, mdns, request_response};
use libp2p::swarm::NetworkBehaviour;

use crate::config::node::{SyncRequest, SyncResponse};

/// The behaviour struct combining all P2P protocols.
/// The `NetworkBehaviour` derive macro automatically generates the `MyBehaviourEvent` enum.
#[derive(NetworkBehaviour)]
pub struct MyBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub identify: identify::Behaviour,
    pub sync: request_response::cbor::Behaviour<SyncRequest, SyncResponse>,
    pub mdns: mdns::tokio::Behaviour,
}
