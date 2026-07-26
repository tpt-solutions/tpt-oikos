use std::collections::{HashSet, VecDeque};

use crate::message::{BlockHeader, NetworkMessage, PeerId, WireTx};
use crate::peer::PeerManager;

pub struct GossipLayer {
    peer_manager: PeerManager,
    message_queue: VecDeque<(PeerId, NetworkMessage)>,
    seen_set: HashSet<[u8; 32]>,
    seen_order: VecDeque<[u8; 32]>,
    max_seen: usize,
}

impl GossipLayer {
    pub fn new(peer_manager: PeerManager, max_seen: usize) -> Self {
        Self {
            peer_manager,
            message_queue: VecDeque::new(),
            seen_set: HashSet::new(),
            seen_order: VecDeque::new(),
            max_seen,
        }
    }

    pub fn peer_manager(&self) -> &PeerManager {
        &self.peer_manager
    }

    pub fn peer_manager_mut(&mut self) -> &mut PeerManager {
        &mut self.peer_manager
    }

    pub fn gossip_block(&mut self, header: &BlockHeader) {
        if self.is_seen(&header.hash) {
            return;
        }
        self.mark_seen(header.hash);

        let msg = NetworkMessage::NewBlock(header.clone());
        let peers = self.peer_manager.get_random_peers(3);
        for peer in peers {
            self.message_queue.push_back((peer.id, msg.clone()));
        }
    }

    pub fn gossip_transaction(&mut self, tx: &WireTx) {
        if self.is_seen(&tx.hash) {
            return;
        }
        self.mark_seen(tx.hash);

        let msg = NetworkMessage::NewTransaction(tx.clone());
        let peers = self.peer_manager.get_random_peers(3);
        for peer in peers {
            self.message_queue.push_back((peer.id, msg.clone()));
        }
    }

    pub fn handle_message(
        &mut self,
        from: PeerId,
        msg: NetworkMessage,
        my_height: u64,
    ) -> Vec<NetworkMessage> {
        let mut responses = Vec::new();

        match &msg {
            NetworkMessage::NewBlock(header) => {
                if !self.is_seen(&header.hash) {
                    self.mark_seen(header.hash);
                    let peers = self.peer_manager.get_random_peers(3);
                    for peer in peers {
                        if peer.id != from {
                            self.message_queue
                                .push_back((peer.id, msg.clone()));
                        }
                    }
                }
            }
            NetworkMessage::NewTransaction(tx) => {
                if !self.is_seen(&tx.hash) {
                    self.mark_seen(tx.hash);
                    let peers = self.peer_manager.get_random_peers(3);
                    for peer in peers {
                        if peer.id != from {
                            self.message_queue
                                .push_back((peer.id, msg.clone()));
                        }
                    }
                }
            }
            NetworkMessage::Ping { node_id: _, height } => {
                self.peer_manager.update_height(&from, *height);
                let my_id = self.peer_manager.my_id();
                responses.push(NetworkMessage::Pong {
                    node_id: my_id,
                    height: my_height,
                });
            }
            NetworkMessage::Pong { node_id: _, height } => {
                self.peer_manager.update_height(&from, *height);
            }
            NetworkMessage::Handshake {
                chain_id,
                node_id: _,
                version: _,
            } => {
                let my_id = self.peer_manager.my_id();
                responses.push(NetworkMessage::HandshakeAck {
                    chain_id: *chain_id,
                    node_id: my_id,
                    best_height: my_height,
                });
            }
            NetworkMessage::HandshakeAck {
                chain_id: _,
                node_id: _,
                best_height,
            } => {
                self.peer_manager.update_height(&from, *best_height);
            }
            NetworkMessage::BlockRequest(_)
            | NetworkMessage::BlockResponse(_)
            | NetworkMessage::TransactionRequest(_)
            | NetworkMessage::TransactionResponse(_)
            | NetworkMessage::StateSyncRequest { .. }
            | NetworkMessage::StateSyncResponse(_) => {}
        }

        responses
    }

    pub fn get_pending_messages(&mut self) -> Vec<(PeerId, NetworkMessage)> {
        self.message_queue.drain(..).collect()
    }

    pub fn is_seen(&self, hash: &[u8; 32]) -> bool {
        self.seen_set.contains(hash)
    }

    pub fn mark_seen(&mut self, hash: [u8; 32]) {
        if self.seen_set.insert(hash) {
            self.seen_order.push_back(hash);
            self.prune_seen();
        }
    }

    pub fn prune_seen(&mut self) {
        while self.seen_order.len() > self.max_seen {
            if let Some(oldest) = self.seen_order.pop_front() {
                self.seen_set.remove(&oldest);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn test_id(n: u8) -> PeerId {
        [n; 32]
    }

    fn test_addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    fn make_gossip(peers: &[(PeerId, SocketAddr)], max_seen: usize) -> GossipLayer {
        let mut pm = PeerManager::new(test_id(0), 100);
        for (id, addr) in peers {
            pm.add_peer(*id, *addr).unwrap();
        }
        GossipLayer::new(pm, max_seen)
    }

    fn dummy_header(height: u64) -> BlockHeader {
        BlockHeader {
            height,
            hash: [height as u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 1_700_000_000,
            validator_id: 1,
            tx_count: 0,
            state_root: [0u8; 32],
        }
    }

    #[test]
    fn gossip_block_enqueues_messages() {
        let peers: Vec<(PeerId, SocketAddr)> = (1u16..=5)
            .map(|i| (test_id(i as u8), test_addr(9000 + i)))
            .collect();
        let mut gossip = make_gossip(&peers, 1000);

        gossip.gossip_block(&dummy_header(1));
        let pending = gossip.get_pending_messages();
        assert_eq!(pending.len(), 3);
    }

    #[test]
    fn gossip_block_deduplicates() {
        let peers = vec![(test_id(1), test_addr(9001))];
        let mut gossip = make_gossip(&peers, 1000);

        let header = dummy_header(1);
        gossip.gossip_block(&header);
        gossip.get_pending_messages();

        gossip.gossip_block(&header);
        let pending = gossip.get_pending_messages();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn gossip_transaction_enqueues() {
        let peers = vec![
            (test_id(1), test_addr(9001)),
            (test_id(2), test_addr(9002)),
        ];
        let mut gossip = make_gossip(&peers, 1000);

        let tx = WireTx {
            hash: [42u8; 32],
            kind: 0,
            sender: 1,
            recipient: 2,
            oikos_amount: 100,
            koin_amount: 0,
            gas_limit: 21000,
            nonce: 1,
            parent_hashes: vec![],
            timestamp: 0,
        };
        gossip.gossip_transaction(&tx);
        let pending = gossip.get_pending_messages();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn handle_new_block_re_gossips() {
        let peers: Vec<(PeerId, SocketAddr)> = (1u16..=3)
            .map(|i| (test_id(i as u8), test_addr(9000 + i)))
            .collect();
        let mut gossip = make_gossip(&peers, 1000);

        let header = dummy_header(1);
        let responses = gossip.handle_message(
            test_id(1),
            NetworkMessage::NewBlock(header),
            0,
        );
        assert!(responses.is_empty());

        let pending = gossip.get_pending_messages();
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().all(|(id, _)| *id != test_id(1)));
    }

    #[test]
    fn handle_new_block_deduplicates() {
        let peers: Vec<(PeerId, SocketAddr)> = (1u16..=3)
            .map(|i| (test_id(i as u8), test_addr(9000 + i)))
            .collect();
        let mut gossip = make_gossip(&peers, 1000);

        let header = dummy_header(1);
        gossip.handle_message(test_id(1), NetworkMessage::NewBlock(header.clone()), 0);
        gossip.get_pending_messages();

        gossip.handle_message(test_id(2), NetworkMessage::NewBlock(header), 0);
        let pending = gossip.get_pending_messages();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn handle_ping_responds_with_pong() {
        let peers = vec![(test_id(1), test_addr(9001))];
        let mut gossip = make_gossip(&peers, 1000);

        let responses = gossip.handle_message(
            test_id(1),
            NetworkMessage::Ping {
                node_id: test_id(1),
                height: 50,
            },
            100,
        );
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            NetworkMessage::Pong { node_id, height } => {
                assert_eq!(*node_id, test_id(0));
                assert_eq!(*height, 100);
            }
            _ => panic!("expected Pong"),
        }
    }

    #[test]
    fn handle_handshake_responds_with_ack() {
        let peers = vec![(test_id(1), test_addr(9001))];
        let mut gossip = make_gossip(&peers, 1000);

        let responses = gossip.handle_message(
            test_id(1),
            NetworkMessage::Handshake {
                chain_id: 1,
                node_id: test_id(1),
                version: 1,
            },
            200,
        );
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            NetworkMessage::HandshakeAck {
                chain_id,
                node_id,
                best_height,
            } => {
                assert_eq!(*chain_id, 1);
                assert_eq!(*node_id, test_id(0));
                assert_eq!(*best_height, 200);
            }
            _ => panic!("expected HandshakeAck"),
        }
    }

    #[test]
    fn mark_seen_and_prune() {
        let pm = PeerManager::new(test_id(0), 10);
        let mut gossip = GossipLayer::new(pm, 3);

        gossip.mark_seen([1u8; 32]);
        gossip.mark_seen([2u8; 32]);
        gossip.mark_seen([3u8; 32]);
        assert!(gossip.is_seen(&[1u8; 32]));

        gossip.mark_seen([4u8; 32]);
        assert_eq!(gossip.seen_order.len(), 3);
        assert!(!gossip.is_seen(&[1u8; 32]));
        assert!(gossip.is_seen(&[4u8; 32]));
    }

    #[test]
    fn get_pending_drains_queue() {
        let peers = vec![(test_id(1), test_addr(9001))];
        let mut gossip = make_gossip(&peers, 1000);

        gossip.gossip_block(&dummy_header(1));
        assert_eq!(gossip.get_pending_messages().len(), 1);
        assert_eq!(gossip.get_pending_messages().len(), 0);
    }
}
