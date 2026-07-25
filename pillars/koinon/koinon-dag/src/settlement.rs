use crate::tx::{TxHash, TxStatus};
use crate::dag::Dag;

#[derive(Debug, Clone)]
pub struct SettlementResult {
    pub settled: Vec<TxHash>,
    pub failed: Vec<TxHash>,
    pub conflicted: Vec<TxHash>,
}

pub struct ParallelSettler;

impl ParallelSettler {
    pub fn settle_batch(dag: &mut Dag, batch: &[TxHash]) -> SettlementResult {
        let mut settled = Vec::new();
        let mut failed = Vec::new();
        let mut conflicted = Vec::new();

        // Collect conflict information first (immutable borrow)
        let conflict_map: Vec<(TxHash, bool)> = batch
            .iter()
            .map(|tx_hash| {
                let has_conflict = dag.get(tx_hash).map_or(false, |node| {
                    dag.settled_nodes().any(|(h, settled_node)| {
                        h != tx_hash && node.tx.conflicts_with(&settled_node.tx)
                    })
                });
                (*tx_hash, has_conflict)
            })
            .collect();

        for (tx_hash, has_conflict) in conflict_map {
            if has_conflict {
                conflicted.push(tx_hash);
                continue;
            }

            if let Some(node) = dag.get(&tx_hash) {
                let deps_ok = node
                    .tx
                    .parent_hashes
                    .iter()
                    .all(|ph| dag.get(ph).map_or(false, |n| n.status == TxStatus::Settled));

                if deps_ok {
                    settled.push(tx_hash);
                } else {
                    failed.push(tx_hash);
                }
            }
        }

        for h in &settled {
            if let Some(node) = dag.get_mut(h) {
                node.status = TxStatus::Settled;
            }
        }
        for h in &failed {
            if let Some(node) = dag.get_mut(h) {
                node.status = TxStatus::Failed;
            }
        }
        for h in &conflicted {
            if let Some(node) = dag.get_mut(h) {
                node.status = TxStatus::Conflict;
            }
        }

        SettlementResult {
            settled,
            failed,
            conflicted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx::{Transaction, TxKind};
    use koinon_ledger::{KoinAmount, OikosAmount};

    fn make_tx(
        hash: u8,
        sender: u64,
        nonce: u64,
        parent_hashes: Vec<TxHash>,
    ) -> Transaction {
        Transaction {
            hash: [hash; 32],
            kind: TxKind::TransferKoin,
            sender,
            recipient: 99,
            oikos_amount: OikosAmount::ZERO,
            koin_amount: KoinAmount(100),
            gas_limit: 21000,
            nonce,
            parent_hashes,
            timestamp: 1,
        }
    }

    #[test]
    fn settle_batch_marks_conflicts() {
        let mut dag = Dag::new();

        let tx_a = make_tx(1, 1, 0, vec![]);
        let tx_a_hash = tx_a.hash;
        dag.insert(tx_a).unwrap();
        let res_a = ParallelSettler::settle_batch(&mut dag, &[tx_a_hash]);
        assert_eq!(res_a.settled.len(), 1);
        assert!(res_a.conflicted.is_empty());

        let tx_b = make_tx(2, 1, 0, vec![tx_a_hash]);
        let tx_b_hash = tx_b.hash;
        dag.insert(tx_b).unwrap();
        let res_b = ParallelSettler::settle_batch(&mut dag, &[tx_b_hash]);
        assert!(res_b.conflicted.contains(&tx_b_hash));
        assert_eq!(res_b.conflicted.len(), 1);
        assert!(dag.get(&tx_b_hash).unwrap().status == TxStatus::Conflict);
    }

    #[test]
    fn settle_batch_no_conflict_different_sender() {
        let mut dag = Dag::new();

        let tx_a = make_tx(1, 1, 0, vec![]);
        let tx_a_hash = tx_a.hash;
        dag.insert(tx_a).unwrap();
        ParallelSettler::settle_batch(&mut dag, &[tx_a_hash]);

        let tx_b = make_tx(2, 2, 0, vec![tx_a_hash]);
        let tx_b_hash = tx_b.hash;
        dag.insert(tx_b).unwrap();
        let res = ParallelSettler::settle_batch(&mut dag, &[tx_b_hash]);
        assert!(res.conflicted.is_empty());
        assert!(res.settled.contains(&tx_b_hash));
    }

    #[test]
    fn settle_batch_no_conflict_different_nonce() {
        let mut dag = Dag::new();

        let tx_a = make_tx(1, 1, 0, vec![]);
        let tx_a_hash = tx_a.hash;
        dag.insert(tx_a).unwrap();
        ParallelSettler::settle_batch(&mut dag, &[tx_a_hash]);

        let tx_b = make_tx(2, 1, 1, vec![tx_a_hash]);
        let tx_b_hash = tx_b.hash;
        dag.insert(tx_b).unwrap();
        let res = ParallelSettler::settle_batch(&mut dag, &[tx_b_hash]);
        assert!(res.conflicted.is_empty());
        assert!(res.settled.contains(&tx_b_hash));
    }

    #[test]
    fn settle_batch_pending_parent_fails() {
        let mut dag = Dag::new();

        let tx_a = make_tx(1, 1, 0, vec![]);
        let tx_a_hash = tx_a.hash;
        dag.insert(tx_a).unwrap();

        let tx_b = make_tx(2, 2, 0, vec![tx_a_hash]);
        let tx_b_hash = tx_b.hash;
        dag.insert(tx_b).unwrap();
        let res = ParallelSettler::settle_batch(&mut dag, &[tx_b_hash]);
        assert!(res.failed.contains(&tx_b_hash));
    }

    #[test]
    fn settle_batch_already_settled_parent_succeeds() {
        let mut dag = Dag::new();

        let tx_a = make_tx(1, 1, 0, vec![]);
        let tx_a_hash = tx_a.hash;
        dag.insert(tx_a).unwrap();
        ParallelSettler::settle_batch(&mut dag, &[tx_a_hash]);

        let tx_b = make_tx(2, 2, 0, vec![tx_a_hash]);
        let tx_b_hash = tx_b.hash;
        dag.insert(tx_b).unwrap();
        let res = ParallelSettler::settle_batch(&mut dag, &[tx_b_hash]);
        assert!(res.settled.contains(&tx_b_hash));
    }

    #[test]
    fn settle_batch_mixed_outcomes() {
        let mut dag = Dag::new();

        let tx_a = make_tx(1, 1, 0, vec![]);
        let tx_a_hash = tx_a.hash;
        dag.insert(tx_a).unwrap();
        ParallelSettler::settle_batch(&mut dag, &[tx_a_hash]);

        let tx_b = make_tx(2, 1, 0, vec![tx_a_hash]);
        let tx_b_hash = tx_b.hash;
        dag.insert(tx_b).unwrap();

        let tx_c = make_tx(3, 2, 0, vec![tx_a_hash]);
        let tx_c_hash = tx_c.hash;
        dag.insert(tx_c).unwrap();

        // tx_d: valid structurally but has pending parent (tx_b was conflicted, not settled)
        let tx_d = make_tx(4, 3, 0, vec![tx_b_hash]);
        let tx_d_hash = tx_d.hash;
        dag.insert(tx_d).unwrap();

        let res = ParallelSettler::settle_batch(&mut dag, &[tx_b_hash, tx_c_hash, tx_d_hash]);
        assert!(res.conflicted.contains(&tx_b_hash));
        assert!(res.settled.contains(&tx_c_hash));
        // tx_d depends on tx_b which was conflicted (not settled), so it fails
        assert!(res.failed.contains(&tx_d_hash));
        assert_eq!(res.settled.len(), 1);
        assert_eq!(res.conflicted.len(), 1);
        assert_eq!(res.failed.len(), 1);
    }

    #[test]
    fn settle_batch_empty() {
        let mut dag = Dag::new();
        let res = ParallelSettler::settle_batch(&mut dag, &[]);
        assert!(res.settled.is_empty());
        assert!(res.failed.is_empty());
        assert!(res.conflicted.is_empty());
    }
}
