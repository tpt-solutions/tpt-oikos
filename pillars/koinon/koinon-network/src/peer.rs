use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rand::seq::SliceRandom;

use crate::message::PeerId;

#[derive(Debug, Clone)]
pub struct Peer {
    pub id: PeerId,
    pub addr: SocketAddr,
    pub best_height: u64,
    pub last_seen: Instant,
    pub connected: bool,
}

pub struct PeerManager {
    peers: HashMap<PeerId, Peer>,
    max_peers: usize,
    my_id: PeerId,
}

impl PeerManager {
    pub fn new(my_id: PeerId, max_peers: usize) -> Self {
        Self {
            peers: HashMap::new(),
            max_peers,
            my_id,
        }
    }

    pub fn my_id(&self) -> PeerId {
        self.my_id
    }

    pub fn add_peer(&mut self, id: PeerId, addr: SocketAddr) -> Result<(), PeerError> {
        if id == self.my_id {
            return Err(PeerError::SelfConnection);
        }
        if self.peers.len() >= self.max_peers && !self.peers.contains_key(&id) {
            return Err(PeerError::MaxPeersReached);
        }
        self.peers.insert(
            id,
            Peer {
                id,
                addr,
                best_height: 0,
                last_seen: Instant::now(),
                connected: true,
            },
        );
        Ok(())
    }

    pub fn remove_peer(&mut self, id: &PeerId) {
        self.peers.remove(id);
    }

    pub fn get_peer(&self, id: &PeerId) -> Option<&Peer> {
        self.peers.get(id)
    }

    pub fn get_peers(&self) -> Vec<&Peer> {
        self.peers.values().collect()
    }

    pub fn get_random_peers(&self, count: usize) -> Vec<&Peer> {
        let mut connected: Vec<&Peer> = self.peers.values().filter(|p| p.connected).collect();
        let mut rng = rand::rng();
        connected.shuffle(&mut rng);
        connected.into_iter().take(count).collect()
    }

    pub fn update_height(&mut self, id: &PeerId, height: u64) {
        if let Some(peer) = self.peers.get_mut(id) {
            peer.best_height = height;
            peer.last_seen = Instant::now();
        }
    }

    pub fn prune_stale(&mut self, timeout: Duration) {
        let now = Instant::now();
        self.peers
            .retain(|_, peer| now.duration_since(peer.last_seen) < timeout);
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum PeerError {
    #[error("cannot add self as peer")]
    SelfConnection,
    #[error("max peers reached")]
    MaxPeersReached,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_id(n: u8) -> PeerId {
        [n; 32]
    }

    fn test_addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    #[test]
    fn add_and_remove_peer() {
        let mut pm = PeerManager::new(test_id(0), 10);
        assert!(pm.add_peer(test_id(1), test_addr(8001)).is_ok());
        assert_eq!(pm.len(), 1);

        pm.remove_peer(&test_id(1));
        assert_eq!(pm.len(), 0);
    }

    #[test]
    fn cannot_add_self() {
        let mut pm = PeerManager::new(test_id(0), 10);
        assert!(matches!(
            pm.add_peer(test_id(0), test_addr(8000)),
            Err(PeerError::SelfConnection)
        ));
    }

    #[test]
    fn max_peers_enforced() {
        let mut pm = PeerManager::new(test_id(0), 2);
        assert!(pm.add_peer(test_id(1), test_addr(8001)).is_ok());
        assert!(pm.add_peer(test_id(2), test_addr(8002)).is_ok());
        assert!(matches!(
            pm.add_peer(test_id(3), test_addr(8003)),
            Err(PeerError::MaxPeersReached)
        ));
    }

    #[test]
    fn re_adding_existing_peer_succeeds() {
        let mut pm = PeerManager::new(test_id(0), 2);
        assert!(pm.add_peer(test_id(1), test_addr(8001)).is_ok());
        assert!(pm.add_peer(test_id(1), test_addr(8001)).is_ok());
        assert_eq!(pm.len(), 1);
    }

    #[test]
    fn get_random_peers_returns_subset() {
        let mut pm = PeerManager::new(test_id(0), 10);
        for i in 1u16..=5 {
            pm.add_peer(test_id(i as u8), test_addr(8000 + i)).unwrap();
        }
        let random = pm.get_random_peers(3);
        assert_eq!(random.len(), 3);
        let ids: Vec<PeerId> = random.iter().map(|p| p.id).collect();
        assert!(ids.windows(2).all(|w| w[0] != w[1]));
    }

    #[test]
    fn update_height() {
        let mut pm = PeerManager::new(test_id(0), 10);
        pm.add_peer(test_id(1), test_addr(8001)).unwrap();
        pm.update_height(&test_id(1), 42);
        assert_eq!(pm.get_peer(&test_id(1)).unwrap().best_height, 42);
    }

    #[test]
    fn prune_stale_removes_old_peers() {
        let mut pm = PeerManager::new(test_id(0), 10);
        pm.add_peer(test_id(1), test_addr(8001)).unwrap();
        pm.add_peer(test_id(2), test_addr(8002)).unwrap();

        // Simulate old peer by backdating last_seen
        pm.peers.get_mut(&test_id(1)).unwrap().last_seen =
            Instant::now() - Duration::from_secs(100);

        pm.prune_stale(Duration::from_secs(60));
        assert_eq!(pm.len(), 1);
        assert!(pm.get_peer(&test_id(2)).is_some());
    }

    #[test]
    fn get_peers_returns_all() {
        let mut pm = PeerManager::new(test_id(0), 10);
        pm.add_peer(test_id(1), test_addr(8001)).unwrap();
        pm.add_peer(test_id(2), test_addr(8002)).unwrap();
        assert_eq!(pm.get_peers().len(), 2);
    }

    #[test]
    fn is_empty_and_len() {
        let mut pm = PeerManager::new(test_id(0), 10);
        assert!(pm.is_empty());
        assert_eq!(pm.len(), 0);
        pm.add_peer(test_id(1), test_addr(8001)).unwrap();
        assert!(!pm.is_empty());
        assert_eq!(pm.len(), 1);
    }
}
