use std::collections::{HashMap, HashSet};
use crate::tx::{Transaction, TxHash, TxStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagError {
    CycleDetected,
    MissingParent(TxHash),
    AlreadyExists(TxHash),
    Conflict(TxHash, TxHash),
}

#[derive(Debug, Clone)]
pub struct DagNode {
    pub tx: Transaction,
    pub status: TxStatus,
    pub children: Vec<TxHash>,
}

#[derive(Debug)]
pub struct Dag {
    nodes: HashMap<TxHash, DagNode>,
    tip_set: HashSet<TxHash>,
}

impl Dag {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            tip_set: HashSet::new(),
        }
    }

    pub fn insert(&mut self, tx: Transaction) -> Result<(), DagError> {
        if self.nodes.contains_key(&tx.hash) {
            return Err(DagError::AlreadyExists(tx.hash));
        }

        for parent_hash in &tx.parent_hashes {
            if !self.nodes.contains_key(parent_hash) {
                return Err(DagError::MissingParent(*parent_hash));
            }
        }

        let node = DagNode {
            tx: tx.clone(),
            status: TxStatus::Pending,
            children: Vec::new(),
        };

        self.tip_set.remove(&tx.hash);
        for parent_hash in &tx.parent_hashes {
            if let Some(parent) = self.nodes.get_mut(parent_hash) {
                parent.children.push(tx.hash);
            }
        }

        self.nodes.insert(tx.hash, node);
        self.tip_set.insert(tx.hash);

        Ok(())
    }

    pub fn get(&self, hash: &TxHash) -> Option<&DagNode> {
        self.nodes.get(hash)
    }

    pub fn get_mut(&mut self, hash: &TxHash) -> Option<&mut DagNode> {
        self.nodes.get_mut(hash)
    }

    pub fn tips(&self) -> impl Iterator<Item = &TxHash> {
        self.tip_set.iter()
    }

    pub fn settled_nodes(&self) -> impl Iterator<Item = (&TxHash, &DagNode)> {
        self.nodes
            .iter()
            .filter(|(_, node)| node.status == TxStatus::Settled)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn parallelizable_groups(&self) -> Vec<Vec<TxHash>> {
        let mut depth_map: HashMap<TxHash, usize> = HashMap::new();
        self.compute_depths(&mut depth_map);

        let mut groups: HashMap<usize, Vec<TxHash>> = HashMap::new();
        for (hash, depth) in &depth_map {
            groups.entry(*depth).or_default().push(*hash);
        }

        let mut result: Vec<Vec<TxHash>> = groups.into_iter().map(|(_, v)| v).collect();
        result.sort_by_key(|g| {
            g.first()
                .and_then(|h| depth_map.get(h))
                .copied()
                .unwrap_or(0)
        });
        result
    }

    fn compute_depths(&self, depths: &mut HashMap<TxHash, usize>) {
        for (hash, node) in &self.nodes {
            self.compute_depth(*hash, node, depths);
        }
    }

    fn compute_depth(
        &self,
        hash: TxHash,
        node: &DagNode,
        depths: &mut HashMap<TxHash, usize>,
    ) {
        if depths.contains_key(&hash) {
            return;
        }
        let max_parent_depth = node
            .tx
            .parent_hashes
            .iter()
            .map(|ph| {
                let parent = &self.nodes[ph];
                self.compute_depth(*ph, parent, depths);
                depths[ph] + 1
            })
            .max()
            .unwrap_or(0);
        depths.insert(hash, max_parent_depth);
    }
}

impl Default for Dag {
    fn default() -> Self {
        Self::new()
    }
}
