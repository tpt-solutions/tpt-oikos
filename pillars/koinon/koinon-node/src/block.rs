use std::collections::HashMap;

use anyhow::Result;
use sha2::{Digest, Sha256};

use koinon_dag::dag::Dag;
use koinon_dag::tx::Transaction;
use koinon_escrow::escrow::Escrow;
use koinon_escrow::streaming::StreamingPayment;
use koinon_gas::pricing::calculate_gas;
use koinon_ledger::{
    Account, KoinAmount, OikosAmount, TotalValueConservation,
};
use koinon_mandates::mandate::AgentMandate;
use koinon_rewards::{BlockReward, BlockRewardConfig, BlockRewardProcessor};
use koinon_staking::staking::StakingPool;
use koinon_treasury::TreasuryPool;

use crate::config::NodeConfig;
use crate::store::StateStore;

/// A finalized block in the chain.
#[derive(Debug, Clone)]
pub struct Block {
    pub height: u64,
    pub hash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    pub validator_id: u64,
    pub reward: BlockReward,
    pub state_root: [u8; 32],
}

impl Block {
    pub fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.height.to_le_bytes());
        hasher.update(self.parent_hash);
        hasher.update(self.timestamp.to_le_bytes());
        hasher.update(self.validator_id.to_le_bytes());
        for tx in &self.transactions {
            hasher.update(tx.hash);
        }
        hasher.update(self.state_root);
        hasher.finalize().into()
    }
}

/// Full in-memory node state.
pub struct NodeState {
    pub dag: Dag,
    pub staking_pool: StakingPool,
    pub treasury: TreasuryPool,
    pub conservation: TotalValueConservation,
    pub reward_processor: BlockRewardProcessor,
    pub accounts: HashMap<u64, Account>,
    pub mandates: Vec<AgentMandate>,
    pub escrows: Vec<Escrow>,
    pub streams: Vec<StreamingPayment>,
}

impl NodeState {
    pub fn compute_state_root(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();

        // Hash accounts
        let mut account_ids: Vec<u64> = self.accounts.keys().copied().collect();
        account_ids.sort();
        for id in &account_ids {
            if let Some(acc) = self.accounts.get(id) {
                hasher.update(id.to_le_bytes());
                hasher.update(acc.oikos_balance.0.to_le_bytes());
                hasher.update(acc.koin_balance.0.to_le_bytes());
                hasher.update(acc.nonce.to_le_bytes());
            }
        }

        // Hash staking
        hasher.update(self.staking_pool.total_staked.0.to_le_bytes());
        let mut validator_ids: Vec<u64> = self.staking_pool.validators.keys().copied().collect();
        validator_ids.sort();
        for vid in &validator_ids {
            if let Some(v) = self.staking_pool.validators.get(vid) {
                hasher.update(vid.to_le_bytes());
                hasher.update(v.staked_amount.0.to_le_bytes());
            }
        }

        // Hash treasury
        hasher.update(self.treasury.balance.0.to_le_bytes());

        // Hash conservation (changes each block due to minting)
        hasher.update(self.conservation.minted.0.to_le_bytes());
        hasher.update(self.conservation.burned.0.to_le_bytes());

        hasher.finalize().into()
    }
}

/// Block production engine.
pub struct BlockProducer {
    config: NodeConfig,
    state: NodeState,
    store: StateStore,
    mempool: Vec<Transaction>,
}

impl BlockProducer {
    /// Create a new block producer, initializing from stored state or genesis.
    pub fn new(config: NodeConfig, store: StateStore) -> Result<Self> {
        let last_height = store.get_last_block_height()?;

        if last_height == 0 {
            let state = Self::genesis_state(&config)?;
            let producer = Self {
                config,
                state,
                store,
                mempool: Vec::new(),
            };
            Ok(producer)
        } else {
            let state = Self::restore_state(&config, &store)?;
            let producer = Self {
                config,
                state,
                store,
                mempool: Vec::new(),
            };
            Ok(producer)
        }
    }

    fn genesis_state(config: &NodeConfig) -> Result<NodeState> {
        let mut accounts = HashMap::new();
        for ga in &config.genesis.initial_accounts {
            accounts.insert(ga.id, ga.to_account());
        }

        let mut staking_pool = StakingPool::new();
        for gv in &config.genesis.initial_validators {
            let id = staking_pool.register_validator(&gv.operator_did)?;
            let stake = OikosAmount(gv.stake);
            if stake.0 > 0 {
                staking_pool.stake(id, stake)?;
            }
        }

        let treasury = TreasuryPool::new(OikosAmount(config.genesis.treasury_balance));

        let mut conservation = TotalValueConservation::new();
        conservation.record_mint(OikosAmount(config.genesis.treasury_balance));

        Ok(NodeState {
            dag: Dag::new(),
            staking_pool,
            treasury,
            conservation,
            reward_processor: BlockRewardProcessor::new(BlockRewardConfig::default()),
            accounts,
            mandates: Vec::new(),
            escrows: Vec::new(),
            streams: Vec::new(),
        })
    }

    fn restore_state(_config: &NodeConfig, store: &StateStore) -> Result<NodeState> {
        let accounts_list = store.list_accounts()?;
        let accounts: HashMap<u64, Account> =
            accounts_list.into_iter().map(|a| (a.id, a)).collect();

        let validator_list = store.list_validators()?;
        let mut staking_pool = StakingPool::new();
        for v in &validator_list {
            staking_pool.register_validator(&v.operator_did)?;
            if let Some(pool_v) = staking_pool.validators.get_mut(&v.id) {
                *pool_v = v.clone();
            }
            staking_pool.total_staked = koinon_ledger::OikosAmount(
                staking_pool
                    .validators
                    .values()
                    .map(|v| v.staked_amount.0)
                    .sum(),
            );
            staking_pool.next_validator_id = staking_pool.next_validator_id.max(v.id + 1);
        }

        let mandates = store.list_mandates()?;

        Ok(NodeState {
            dag: Dag::new(),
            staking_pool,
            treasury: TreasuryPool::new(OikosAmount::ZERO),
            conservation: TotalValueConservation::new(),
            reward_processor: BlockRewardProcessor::new(BlockRewardConfig::default()),
            accounts,
            mandates,
            escrows: Vec::new(),
            streams: Vec::new(),
        })
    }

    /// Submit a transaction to the mempool.
    pub fn submit_transaction(&mut self, tx: Transaction) -> Result<[u8; 32]> {
        let hash = tx.hash;
        self.mempool.push(tx);
        Ok(hash)
    }

    /// Produce a new block at the given time.
    pub fn produce_block(&mut self, current_time: u64) -> Result<Block> {
        let height = self.store.get_last_block_height()? + 1;

        let parent_hash = if height == 1 {
            [0u8; 32]
        } else {
            self.store
                .get_block(height - 1)?
                .map(|b| b.hash)
                .unwrap_or([0u8; 32])
        };

        // Select transactions from mempool (simple: take all pending)
        let block_txs: Vec<Transaction> = self.mempool.drain(..).collect();

        // Calculate gas fees from transactions
        let total_gas_fees: i128 = block_txs
            .iter()
            .map(|tx| {
                let gas_cost = calculate_gas(tx.gas_limit, 0);
                gas_cost.as_u64() as i128
            })
            .sum();

        // Process block reward
        let reward = self
            .state
            .reward_processor
            .process_block(height, KoinAmount(total_gas_fees))?;

        // Track mint in conservation
        if reward.base_reward.0 > 0 {
            self.state.conservation.record_mint(reward.base_reward);
        }

        // Distribute validator rewards
        self.state.staking_pool.distribute_rewards(reward.base_reward);

        // Select validator (first active validator for simplicity)
        let validator_id = self
            .state
            .staking_pool
            .active_validators()
            .first()
            .map(|v| v.id)
            .unwrap_or(0);

        let state_root = self.state.compute_state_root();

        let mut block = Block {
            height,
            hash: [0u8; 32],
            parent_hash,
            timestamp: current_time,
            transactions: block_txs,
            validator_id,
            reward,
            state_root,
        };
        block.hash = block.compute_hash();

        // Persist to store
        self.store.apply_block(&block)?;

        // Persist updated validators
        for v in self.state.staking_pool.validators.values() {
            self.store.upsert_validator(v)?;
        }

        log::info!(
            "Block #{} produced | hash={} | txs={} | validator={}",
            block.height,
            hex_short(&block.hash),
            block.transactions.len(),
            block.validator_id,
        );

        Ok(block)
    }

    pub fn get_state(&self) -> &NodeState {
        &self.state
    }

    pub fn get_state_mut(&mut self) -> &mut NodeState {
        &mut self.state
    }

    pub fn mempool_size(&self) -> usize {
        self.mempool.len()
    }
}

fn hex_short(bytes: &[u8; 32]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}...{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[28], bytes[29], bytes[30], bytes[31]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use koinon_dag::tx::TxKind;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_store() -> StateStore {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "koinon_node_block_test_{}_{}",
            std::process::id(), id
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_blocks.db");
        let _ = std::fs::remove_file(&path);
        StateStore::new(path.to_str().unwrap()).unwrap()
    }

    #[test]
    fn genesis_block_production() {
        let store = temp_store();
        let config = NodeConfig::default();
        let mut producer = BlockProducer::new(config, store).unwrap();
        let block = producer.produce_block(1000).unwrap();
        assert_eq!(block.height, 1);
        assert_eq!(block.parent_hash, [0u8; 32]);
        assert!(block.reward.base_reward.0 > 0);
        assert_ne!(block.hash, [0u8; 32]);
    }

    #[test]
    fn sequential_blocks() {
        let store = temp_store();
        let config = NodeConfig::default();
        let mut producer = BlockProducer::new(config, store).unwrap();

        let b1 = producer.produce_block(1000).unwrap();
        assert_eq!(b1.height, 1);

        let b2 = producer.produce_block(2000).unwrap();
        assert_eq!(b2.height, 2);
        assert_eq!(b2.parent_hash, b1.hash);

        let b3 = producer.produce_block(3000).unwrap();
        assert_eq!(b3.height, 3);
        assert_eq!(b3.parent_hash, b2.hash);
    }

    #[test]
    fn state_root_changes_with_state() {
        let store = temp_store();
        let config = NodeConfig::default();
        let mut producer = BlockProducer::new(config, store).unwrap();

        let root1 = producer.get_state().compute_state_root();
        let _block = producer.produce_block(1000).unwrap();
        let root2 = producer.get_state().compute_state_root();

        // State root should change after block production (reward distributed)
        assert_ne!(root1, root2);
    }

    #[test]
    fn submit_transaction_adds_to_mempool() {
        let store = temp_store();
        let config = NodeConfig::default();
        let mut producer = BlockProducer::new(config, store).unwrap();
        assert_eq!(producer.mempool_size(), 0);

        let tx = Transaction {
            hash: [0xAA; 32],
            kind: TxKind::TransferKoin,
            sender: 1,
            recipient: 2,
            oikos_amount: OikosAmount::ZERO,
            koin_amount: KoinAmount(100),
            gas_limit: 21000,
            nonce: 0,
            parent_hashes: vec![],
            timestamp: 500,
        };
        producer.submit_transaction(tx).unwrap();
        assert_eq!(producer.mempool_size(), 1);
    }

    #[test]
    fn block_includes_mempool_transactions() {
        let store = temp_store();
        let config = NodeConfig::default();
        let mut producer = BlockProducer::new(config, store).unwrap();

        let tx = Transaction {
            hash: [0xBB; 32],
            kind: TxKind::TransferOikos,
            sender: 1,
            recipient: 2,
            oikos_amount: OikosAmount(100),
            koin_amount: KoinAmount::ZERO,
            gas_limit: 21000,
            nonce: 0,
            parent_hashes: vec![],
            timestamp: 500,
        };
        producer.submit_transaction(tx).unwrap();
        let block = producer.produce_block(1000).unwrap();
        assert_eq!(block.transactions.len(), 1);
        assert_eq!(block.transactions[0].hash, [0xBB; 32]);
    }

    #[test]
    fn block_hash_is_nonzero() {
        let store = temp_store();
        let config = NodeConfig::default();
        let mut producer = BlockProducer::new(config, store).unwrap();
        let block = producer.produce_block(1000).unwrap();
        assert_ne!(block.hash, [0u8; 32]);
    }
}
