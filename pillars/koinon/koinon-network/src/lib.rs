pub mod gossip;
pub mod message;
pub mod peer;
pub mod transport;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Context;
use message::{BlockHeader, NetworkMessage, PeerId, WireTx};
use peer::PeerManager;
use transport::{ConnId, TcpTransport};

pub use gossip::GossipLayer;
pub use message::{Block, TxHash};
pub use peer::PeerError;
pub use transport::TcpTransport as Transport;

const PROTOCOL_VERSION: u32 = 1;
const MAX_SEEN_MESSAGES: usize = 10_000;

pub struct NetworkConfig {
    pub chain_id: u64,
    pub bind_addr: SocketAddr,
    pub seed_nodes: Vec<SocketAddr>,
    pub max_peers: usize,
    pub peer_timeout_secs: u64,
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    BlockReceived(BlockHeader),
    TransactionReceived(WireTx),
    PeerConnected(PeerId, SocketAddr),
    PeerDisconnected(PeerId),
    SyncNeeded(u64),
}

pub struct NetworkNode {
    transport: TcpTransport,
    gossip: GossipLayer,
    my_id: PeerId,
    chain_id: u64,
    best_height: AtomicU64,
    shutdown: AtomicBool,
    conn_to_peer: std::collections::HashMap<ConnId, PeerId>,
    peer_to_conn: std::collections::HashMap<PeerId, ConnId>,
    config: NetworkConfig,
}

impl NetworkNode {
    pub async fn new(config: NetworkConfig) -> anyhow::Result<Self> {
        let my_id = generate_node_id();
        let transport = TcpTransport::new(config.bind_addr)
            .await
            .context("failed to create TCP transport")?;
        let peer_manager = PeerManager::new(my_id, config.max_peers);
        let gossip = GossipLayer::new(peer_manager, MAX_SEEN_MESSAGES);

        Ok(Self {
            transport,
            gossip,
            my_id,
            chain_id: config.chain_id,
            best_height: AtomicU64::new(0),
            shutdown: AtomicBool::new(false),
            conn_to_peer: std::collections::HashMap::new(),
            peer_to_conn: std::collections::HashMap::new(),
            config,
        })
    }

    pub fn my_id(&self) -> PeerId {
        self.my_id
    }

    pub fn best_height(&self) -> u64 {
        self.best_height.load(Ordering::Relaxed)
    }

    pub fn set_best_height(&self, height: u64) {
        self.best_height.store(height, Ordering::Relaxed);
    }

    pub async fn connect_to_seed(&mut self, seed_addr: SocketAddr) -> anyhow::Result<()> {
        let conn_id = self.transport.connect(seed_addr).await?;
        let handshake = NetworkMessage::Handshake {
            chain_id: self.chain_id,
            node_id: self.my_id,
            version: PROTOCOL_VERSION,
        };
        self.transport.send(conn_id, &handshake).await?;
        log::info!("sent handshake to seed {seed_addr}");
        Ok(())
    }

    pub fn broadcast_block(&mut self, header: &BlockHeader) {
        self.gossip.gossip_block(header);
        let height = header.height;
        if height > self.best_height.load(Ordering::Relaxed) {
            self.best_height.store(height, Ordering::Relaxed);
        }
    }

    pub fn broadcast_transaction(&mut self, tx: &WireTx) {
        self.gossip.gossip_transaction(tx);
    }

    pub async fn poll(&mut self) -> Vec<NetworkEvent> {
        if self.shutdown.load(Ordering::Relaxed) {
            return Vec::new();
        }

        // Accept incoming connections
        if let Ok(Some((conn_id, addr))) = self.transport.accept_one().await {
            log::info!("accepted connection from {addr}");
            let handshake = NetworkMessage::Handshake {
                chain_id: self.chain_id,
                node_id: self.my_id,
                version: PROTOCOL_VERSION,
            };
            let _ = self.transport.send(conn_id, &handshake).await;
        }

        // Receive raw messages from transport
        let raw_messages = self.transport.poll();
        let mut events = Vec::new();
        let my_height = self.best_height.load(Ordering::Relaxed);

        for (conn_id, msg) in raw_messages {
            let from_peer = self.conn_to_peer.get(&copied(conn_id)).copied();
            let responses = if let Some(peer_id) = from_peer {
                self.gossip.handle_message(peer_id, msg, my_height)
            } else {
                // Unknown connection — only accept Handshake
                match &msg {
                    NetworkMessage::Handshake {
                        chain_id,
                        node_id,
                        version,
                    } => {
                        if *chain_id != self.chain_id {
                            log::warn!(
                                "chain_id mismatch: expected {}, got {}",
                                self.chain_id,
                                chain_id
                            );
                            continue;
                        }
                        if *version != PROTOCOL_VERSION {
                            log::warn!(
                                "version mismatch: expected {}, got {}",
                                PROTOCOL_VERSION,
                                version
                            );
                            continue;
                        }
                        let peer_id = *node_id;
                        if let Err(e) = self.gossip.peer_manager_mut().add_peer(
                            peer_id,
                            self.transport
                                .addr_for_connection(conn_id)
                                .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap()),
                        ) {
                            log::warn!("failed to add peer: {e}");
                            continue;
                        }
                        self.conn_to_peer.insert(conn_id, peer_id);
                        self.peer_to_conn.insert(peer_id, conn_id);
                        log::info!("registered peer {:?} from conn {}", hex_short(&peer_id), conn_id);

                        let _remote_height = 0;
                        events.push(NetworkEvent::PeerConnected(
                            peer_id,
                            self.transport
                                .addr_for_connection(conn_id)
                                .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap()),
                        ));

                        vec![NetworkMessage::HandshakeAck {
                            chain_id: self.chain_id,
                            node_id: self.my_id,
                            best_height: my_height,
                        }]
                    }
                    NetworkMessage::HandshakeAck {
                        chain_id,
                        node_id,
                        best_height,
                    } => {
                        if *chain_id != self.chain_id {
                            continue;
                        }
                        let peer_id = *node_id;
                        if let Err(e) = self.gossip.peer_manager_mut().add_peer(
                            peer_id,
                            self.transport
                                .addr_for_connection(conn_id)
                                .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap()),
                        ) {
                            log::warn!("failed to add peer from ack: {e}");
                            continue;
                        }
                        self.conn_to_peer.insert(conn_id, peer_id);
                        self.peer_to_conn.insert(peer_id, conn_id);

                        self.gossip
                            .peer_manager_mut()
                            .update_height(&peer_id, *best_height);
                        events.push(NetworkEvent::PeerConnected(
                            peer_id,
                            self.transport
                                .addr_for_connection(conn_id)
                                .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap()),
                        ));

                        if *best_height > my_height {
                            events.push(NetworkEvent::SyncNeeded(*best_height));
                        }

                        Vec::new()
                    }
                    _ => {
                        log::debug!("ignoring message from unknown connection {conn_id}");
                        continue;
                    }
                }
            };

            // Forward application-level responses
            for resp in &responses {
                match resp {
                    NetworkMessage::NewBlock(header) => {
                        events.push(NetworkEvent::BlockReceived(header.clone()));
                    }
                    NetworkMessage::NewTransaction(tx) => {
                        events.push(NetworkEvent::TransactionReceived(tx.clone()));
                    }
                    _ => {}
                }
            }

            // Send protocol responses back through transport
            for resp in responses {
                let _ = self.transport.send(conn_id, &resp).await;
            }
        }

        // Drain gossip queue and send
        let pending = self.gossip.get_pending_messages();
        for (peer_id, msg) in pending {
            if let Some(&conn_id) = self.peer_to_conn.get(&peer_id) {
                let _ = self.transport.send(conn_id, &msg).await;
            }
        }

        // Prune stale peers
        self.gossip.peer_manager_mut().prune_stale(Duration::from_secs(self.config.peer_timeout_secs));

        // Detect disconnected peers
        let connected_peers: Vec<PeerId> = self
            .gossip
            .peer_manager()
            .get_peers()
            .iter()
            .map(|p| p.id)
            .collect();
        for peer_id in connected_peers {
            if self.peer_to_conn.get(&peer_id).is_none() {
                self.gossip.peer_manager_mut().remove_peer(&peer_id);
                events.push(NetworkEvent::PeerDisconnected(peer_id));
            }
        }

        events
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        log::info!("network node shutting down");
    }

    pub fn peer_count(&self) -> usize {
        self.gossip.peer_manager().len()
    }
}

fn generate_node_id() -> PeerId {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let s = RandomState::new();
    let mut hasher = s.build_hasher();
    hasher.write_u64(std::process::id() as u64);
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );
    let h = hasher.finish();
    let mut id = [0u8; 32];
    id[..8].copy_from_slice(&h.to_le_bytes());
    id
}

fn hex_short(bytes: &[u8; 32]) -> String {
    bytes[..4]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

fn copied(conn_id: ConnId) -> ConnId {
    conn_id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(port: u16) -> NetworkConfig {
        NetworkConfig {
            chain_id: 1,
            bind_addr: SocketAddr::from(([127, 0, 0, 1], port)),
            seed_nodes: vec![],
            max_peers: 10,
            peer_timeout_secs: 60,
        }
    }

    #[tokio::test]
    async fn node_start_and_shutdown() {
        let config = test_config(0);
        let node = NetworkNode::new(config).await.unwrap();
        assert_eq!(node.peer_count(), 0);
        node.shutdown();
        assert!(node.shutdown.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn two_nodes_handshake() {
        let mut node1 = NetworkNode::new(test_config(0)).await.unwrap();
        let addr1 = node1.transport.local_addr().unwrap();

        let mut node2 = NetworkNode::new(test_config(0)).await.unwrap();
        node2.connect_to_seed(addr1).await.unwrap();

        // Poll both sides until handshake completes (up to 50 iterations)
        for _ in 0..50 {
            let _ = node1.poll().await;
            let _ = node2.poll().await;
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            if node1.peer_count() > 0 && node2.peer_count() > 0 {
                break;
            }
        }

        assert!(node1.peer_count() > 0 && node2.peer_count() > 0);
    }

    #[tokio::test]
    async fn broadcast_block_generates_events() {
        let config = test_config(0);
        let mut node = NetworkNode::new(config).await.unwrap();

        let header = BlockHeader {
            height: 1,
            hash: [1u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 1_700_000_000,
            validator_id: 1,
            tx_count: 0,
            state_root: [0u8; 32],
        };

        node.broadcast_block(&header);
        assert_eq!(node.best_height(), 1);
    }

    #[tokio::test]
    async fn connect_to_nonexistent_seed_fails() {
        let mut node = NetworkNode::new(test_config(0)).await.unwrap();
        let result = node
            .connect_to_seed("127.0.0.1:59999".parse().unwrap())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn network_config_values() {
        let config = NetworkConfig {
            chain_id: 42,
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            seed_nodes: vec!["127.0.0.1:9000".parse().unwrap()],
            max_peers: 50,
            peer_timeout_secs: 300,
        };
        assert_eq!(config.chain_id, 42);
        assert_eq!(config.max_peers, 50);
    }
}
