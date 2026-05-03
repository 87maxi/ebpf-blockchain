// ============================================================================
// P3-2: Test Suite - Unit Tests for Security and Consensus
// ============================================================================

use crate::config::node::{Transaction, Vote, Block, CHECKPOINT_INTERVAL, get_current_timestamp};
use crate::security::replay::ReplayProtection;
use crate::security::sybil::SybilProtection;
use crate::security::eclipse::EclipseProtection;
use rocksdb::DB;
use tempfile::TempDir;
use std::sync::Arc;
use std::net::Ipv4Addr;
use ed25519_dalek::{Signer, VerifyingKey, SigningKey};

// ---------------------------------------------------------------------------
// Replay Protection Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod replay_protection_tests {
    use super::*;

    fn create_test_db() -> Arc<DB> {
        let temp_dir = TempDir::new().unwrap();
        Arc::new(DB::open_default(temp_dir.path()).unwrap())
    }

    #[test]
    fn test_first_nonce_accepted() {
        let db = create_test_db();
        let protection = ReplayProtection::new(db);

        // First nonce from a sender should be accepted
        let result = protection.validate_nonce("sender1", 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2); // Next expected nonce
    }

    #[test]
    fn test_incremental_nonce_accepted() {
        let db = create_test_db();
        let protection = ReplayProtection::new(db);

        // Set initial nonce
        protection.update_nonce("sender1", 5).unwrap();

        // Next nonce should be accepted
        let result = protection.validate_nonce("sender1", 6);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 7);
    }

    #[test]
    fn test_duplicate_nonce_rejected() {
        let db = create_test_db();
        let protection = ReplayProtection::new(db);

        // Set initial nonce
        protection.update_nonce("sender1", 5).unwrap();

        // Duplicate nonce should be rejected
        let result = protection.validate_nonce("sender1", 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_decreasing_nonce_rejected() {
        let db = create_test_db();
        let protection = ReplayProtection::new(db);

        // Set initial nonce
        protection.update_nonce("sender1", 10).unwrap();

        // Decreasing nonce should be rejected
        let result = protection.validate_nonce("sender1", 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_deduplication() {
        let db = create_test_db();
        let protection = ReplayProtection::new(db);

        let tx_id = "tx-abc-123".to_string();

        // First time - not processed
        assert!(!protection.is_processed(&tx_id));

        // Mark as processed
        protection.mark_processed(&tx_id, 1000).unwrap();

        // Now should be processed
        assert!(protection.is_processed(&tx_id));
    }

    #[test]
    fn test_timestamp_validation() {
        // Valid timestamp (current)
        let tx = Transaction::new("tx1".to_string(), "data".to_string(), 1);
        assert!(tx.is_timestamp_valid());

        // Old timestamp (should be invalid)
        let old_tx = Transaction {
            id: "tx2".to_string(),
            data: "data".to_string(),
            nonce: 1,
            timestamp: 0, // Unix epoch - definitely expired
        };
        assert!(!old_tx.is_timestamp_valid());
    }
}

// ---------------------------------------------------------------------------
// Sybil Protection Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod sybil_protection_tests {
    use super::*;
    use libp2p::PeerId;

    fn create_test_db() -> Arc<DB> {
        let temp_dir = TempDir::new().unwrap();
        Arc::new(DB::open_default(temp_dir.path()).unwrap())
    }

    #[test]
    fn test_ip_limit_enforcement() {
        let db = create_test_db();
        let protection = SybilProtection::new(db, 3); // Max 3 per IP

        let ip = Ipv4Addr::new(192, 168, 1, 100);
        let peer1: PeerId = "12D3KooWShell1".parse().unwrap_or_else(|_| PeerId::random());
        let peer2: PeerId = "12D3KooWShell2".parse().unwrap_or_else(|_| PeerId::random());
        let peer3: PeerId = PeerId::random();
        let peer4: PeerId = PeerId::random();

        // First 3 connections should be allowed
        assert!(protection.register_connection(peer1, &ip).is_ok());
        assert!(protection.check_ip_limit(peer1, &ip).is_ok());

        assert!(protection.register_connection(peer2, &ip).is_ok());
        assert!(protection.check_ip_limit(peer2, &ip).is_ok());

        assert!(protection.register_connection(peer3, &ip).is_ok());
        // After 3 connections, the 4th should fail
        assert!(protection.check_ip_limit(peer4, &ip).is_err());
    }

    #[test]
    fn test_connection_counting() {
        let db = create_test_db();
        let protection = SybilProtection::new(db, 5);

        let ip = Ipv4Addr::new(10, 0, 0, 1);
        let peer1: PeerId = PeerId::random();
        let peer2: PeerId = PeerId::random();

        protection.register_connection(peer1, &ip).unwrap();
        protection.register_connection(peer2, &ip).unwrap();

        assert_eq!(protection.count_connections_per_ip(&ip), 2);
    }

    #[test]
    fn test_connection_unregistration() {
        let db = create_test_db();
        let protection = SybilProtection::new(db, 5);

        let ip = Ipv4Addr::new(10, 0, 0, 1);
        let peer: PeerId = PeerId::random();

        protection.register_connection(peer, &ip).unwrap();
        assert_eq!(protection.count_connections_per_ip(&ip), 1);

        protection.unregister_connection(peer, &ip).unwrap();
        assert_eq!(protection.count_connections_per_ip(&ip), 0);
    }

    #[test]
    fn test_whitelist_operations() {
        let db = create_test_db();
        let protection = SybilProtection::new(db, 5);

        let peer: PeerId = PeerId::random();

        assert_eq!(protection.get_whitelisted_peer_count(), 0);

        protection.add_to_whitelist(peer).unwrap();
        assert_eq!(protection.get_whitelisted_peer_count(), 1);

        let whitelisted = protection.get_whitelisted_peers();
        assert!(whitelisted.contains(&peer));

        protection.remove_from_whitelist(peer).unwrap();
        assert_eq!(protection.get_whitelisted_peer_count(), 0);
    }
}

// ---------------------------------------------------------------------------
// Eclipse Protection Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod eclipse_protection_tests {
    use super::*;
    use libp2p::PeerId;

    fn create_test_db() -> Arc<DB> {
        let temp_dir = TempDir::new().unwrap();
        Arc::new(DB::open_default(temp_dir.path()).unwrap())
    }

    #[test]
    fn test_no_eclipse_risk_with_diverse_peers() {
        let db = create_test_db();
        let protection = EclipseProtection::new(db);

        // Register peers from different /24 networks
        let peer1: PeerId = PeerId::random();
        let peer2: PeerId = PeerId::random();
        let peer3: PeerId = PeerId::random();

        protection.register_peer(peer1, "192.168.1.10").unwrap();
        protection.register_peer(peer2, "10.0.0.5").unwrap();
        protection.register_peer(peer3, "172.16.5.20").unwrap();

        let (score, prefixes, peers) = protection.calculate_risk_score();

        assert_eq!(peers, 3);
        assert_eq!(prefixes, 3);
        assert!(score < 30.0, "Low risk expected with diverse peers, got {}", score);
    }

    #[test]
    fn test_high_eclipse_risk_with_single_prefix() {
        let db = create_test_db();
        let protection = EclipseProtection::new(db);

        // All peers from same /24 network
        let peer1: PeerId = PeerId::random();
        let peer2: PeerId = PeerId::random();
        let peer3: PeerId = PeerId::random();

        protection.register_peer(peer1, "192.168.1.10").unwrap();
        protection.register_peer(peer2, "192.168.1.20").unwrap();
        protection.register_peer(peer3, "192.168.1.30").unwrap();

        let (score, prefixes, peers) = protection.calculate_risk_score();

        assert_eq!(peers, 3);
        assert_eq!(prefixes, 1);
        assert!(score > 30.0, "High risk expected with single prefix, got {}", score);
    }

    #[test]
    fn test_eclipse_risk_with_few_peers() {
        let db = create_test_db();
        let protection = EclipseProtection::new(db);

        // Only 1 peer connected
        let peer1: PeerId = PeerId::random();
        protection.register_peer(peer1, "192.168.1.10").unwrap();

        let (score, prefixes, peers) = protection.calculate_risk_score();

        assert_eq!(peers, 1);
        assert!(score > 30.0, "High risk expected with single peer, got {}", score);
    }

    #[test]
    fn test_peer_unregistration() {
        let db = create_test_db();
        let protection = EclipseProtection::new(db);

        let peer: PeerId = PeerId::random();
        protection.register_peer(peer, "192.168.1.10").unwrap();
        assert_eq!(protection.calculate_risk_score().2, 1);

        protection.unregister_peer(peer).unwrap();
        assert_eq!(protection.calculate_risk_score().2, 0);
    }
}

// ---------------------------------------------------------------------------
// Block Hash Verification Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod block_tests {
    use super::*;

    #[test]
    fn test_block_hash_consistency() {
        let block = Block {
            height: 1,
            hash: String::new(),
            parent_hash: "genesis".to_string(),
            proposer: "proposer1".to_string(),
            timestamp: get_current_timestamp(),
            transactions: vec!["tx1".to_string(), "tx2".to_string()],
            quorum_votes: 3,
            total_validators: 5,
        };

        let hash1 = block.compute_hash();
        let hash2 = block.compute_hash();

        assert_eq!(hash1, hash2, "Hash should be deterministic");
        assert!(!hash1.is_empty(), "Hash should not be empty");
    }

    #[test]
    fn test_block_hash_uniqueness() {
        let block1 = Block {
            height: 1,
            hash: String::new(),
            parent_hash: "genesis".to_string(),
            proposer: "proposer1".to_string(),
            timestamp: get_current_timestamp(),
            transactions: vec!["tx1".to_string()],
            quorum_votes: 3,
            total_validators: 5,
        };

        let block2 = Block {
            height: 2,
            hash: String::new(),
            parent_hash: "genesis".to_string(),
            proposer: "proposer1".to_string(),
            timestamp: get_current_timestamp(),
            transactions: vec!["tx1".to_string()],
            quorum_votes: 3,
            total_validators: 5,
        };

        assert_ne!(
            block1.compute_hash(),
            block2.compute_hash(),
            "Different blocks should have different hashes"
        );
    }

    #[test]
    fn test_genesis_block_structure() {
        let genesis = Block {
            height: 0,
            hash: String::new(),
            parent_hash: "0x0".to_string(),
            proposer: "genesis".to_string(),
            timestamp: get_current_timestamp(),
            transactions: vec![],
            quorum_votes: 0,
            total_validators: 0,
        };

        assert_eq!(genesis.height, 0);
        assert_eq!(genesis.parent_hash, "0x0");
        assert!(genesis.transactions.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Ed25519 Signature Verification Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod signature_tests {
    use super::*;

    #[test]
    fn test_vote_signature_verification() {
        // Generate a deterministic signing key from a fixed seed
        let seed = [42u8; 32];
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = VerifyingKey::from(&signing_key);

        let vote = Vote::new(
            "tx-123".to_string(),
            "voter-abc".to_string(),
            "validator-xyz".to_string(),
        );

        let signature = signing_key.sign(vote.to_bytes().as_slice());

        // Verify signature
        let result = verifying_key.verify_strict(
            &vote.to_bytes(),
            &signature,
        );
        assert!(result.is_ok(), "Valid signature should verify");
    }

    #[test]
    fn test_tampered_vote_rejected() {
        // Generate a deterministic signing key from a fixed seed
        let seed = [42u8; 32];
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = VerifyingKey::from(&signing_key);

        let vote = Vote::new(
            "tx-123".to_string(),
            "voter-abc".to_string(),
            "validator-xyz".to_string(),
        );

        let signature = signing_key.sign(vote.to_bytes().as_slice());

        // Tamper with the vote
        let tampered_vote = Vote::new(
            "tx-456".to_string(), // Different tx_id
            "voter-abc".to_string(),
            "validator-xyz".to_string(),
        );

        // Signature should not verify for tampered vote
        let result = verifying_key.verify_strict(
            &tampered_vote.to_bytes(),
            &signature,
        );
        assert!(result.is_err(), "Tampered vote should fail verification");
    }

    #[test]
    fn test_vote_byte_consistency() {
        let vote = Vote::new(
            "tx-123".to_string(),
            "voter-abc".to_string(),
            "validator-xyz".to_string(),
        );

        let bytes1 = vote.to_bytes();
        let bytes2 = vote.to_bytes();

        assert_eq!(bytes1, bytes2, "Vote bytes should be consistent");
    }
}

// ---------------------------------------------------------------------------
// Consensus Quorum Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod consensus_tests {
    use crate::config::node::CHECKPOINT_INTERVAL;

    #[test]
    fn test_quorum_calculation() {
        // 2/3 majority with rounding up: (total * 2 + 2) / 3
        let test_cases = vec![
            (1, 1),   // 1 validator → 1 vote needed
            (2, 2),   // 2 validators → 2 votes needed
            (3, 2),   // 3 validators → 2 votes needed
            (4, 3),   // 4 validators → 3 votes needed
            (5, 4),   // 5 validators → 4 votes needed
            (10, 7),  // 10 validators → 7 votes needed
        ];

        for (total, expected) in test_cases {
            let quorum = (total * 2 + 2) / 3;
            assert_eq!(
                quorum, expected,
                "Quorum for {} validators should be {}, got {}",
                total, expected, quorum
            );
        }
    }

    #[test]
    fn test_checkpoint_interval() {
        assert_eq!(CHECKPOINT_INTERVAL, 100);

        // Heights that should trigger checkpoints
        assert!(100 % CHECKPOINT_INTERVAL == 0);
        assert!(200 % CHECKPOINT_INTERVAL == 0);
        assert!(1000 % CHECKPOINT_INTERVAL == 0);

        // Heights that should not
        assert!(99 % CHECKPOINT_INTERVAL != 0);
        assert!(150 % CHECKPOINT_INTERVAL != 0);
    }
}
