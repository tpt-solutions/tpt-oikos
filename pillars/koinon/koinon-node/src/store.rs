use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use koinon_dag::tx::{Transaction, TxHash, TxKind};
use koinon_ledger::{Account, AccountId, KoinAmount, OikosAmount};
use koinon_mandates::mandate::AgentMandate;
use koinon_mandates::scope::MandateScope;
use koinon_staking::staking::Validator;

use crate::block::Block;

/// SQLite-backed persistent state store.
pub struct StateStore {
    conn: Connection,
}

impl StateStore {
    /// Open or create the database at the given path.
    pub fn new(path: &str) -> Result<Self> {
        let parent = Path::new(path).parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)
            .context("failed to create data directory")?;

        let conn = Connection::open(path).context("failed to open database")?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .context("failed to set pragmas")?;

        let store = Self { conn };
        store.create_tables()?;
        Ok(store)
    }

    fn create_tables(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY,
                oikos_balance TEXT NOT NULL DEFAULT '0',
                koin_balance TEXT NOT NULL DEFAULT '0',
                nonce INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS validators (
                id INTEGER PRIMARY KEY,
                operator_did TEXT NOT NULL,
                staked_amount TEXT NOT NULL DEFAULT '0',
                reward_debt TEXT NOT NULL DEFAULT '0',
                active INTEGER NOT NULL DEFAULT 1,
                slashed_amount TEXT NOT NULL DEFAULT '0',
                created_at INTEGER NOT NULL DEFAULT 0,
                jailed_until INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS transactions (
                hash BLOB PRIMARY KEY,
                kind TEXT NOT NULL,
                sender INTEGER NOT NULL,
                recipient INTEGER NOT NULL,
                oikos_amount TEXT NOT NULL DEFAULT '0',
                koin_amount TEXT NOT NULL DEFAULT '0',
                gas_limit INTEGER NOT NULL DEFAULT 0,
                nonce INTEGER NOT NULL DEFAULT 0,
                parent_hashes TEXT NOT NULL DEFAULT '[]',
                timestamp INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'Pending'
            );

            CREATE TABLE IF NOT EXISTS mandates (
                id INTEGER PRIMARY KEY,
                principal_did TEXT NOT NULL,
                agent_did TEXT NOT NULL,
                oikos_budget TEXT NOT NULL DEFAULT '0',
                koin_budget TEXT NOT NULL DEFAULT '0',
                scopes TEXT NOT NULL DEFAULT '[]',
                time_bound INTEGER,
                active INTEGER NOT NULL DEFAULT 1,
                oikos_spent TEXT NOT NULL DEFAULT '0',
                koin_spent TEXT NOT NULL DEFAULT '0'
            );

            CREATE TABLE IF NOT EXISTS escrows (
                id INTEGER PRIMARY KEY,
                sender INTEGER NOT NULL,
                receiver INTEGER NOT NULL,
                oikos_amount TEXT NOT NULL DEFAULT '0',
                koin_amount TEXT NOT NULL DEFAULT '0',
                state TEXT NOT NULL DEFAULT 'Funded',
                conditions TEXT NOT NULL DEFAULT '[]'
            );

            CREATE TABLE IF NOT EXISTS streams (
                id INTEGER PRIMARY KEY,
                sender INTEGER NOT NULL,
                receiver INTEGER NOT NULL,
                total_amount INTEGER NOT NULL DEFAULT 0,
                rate_per_second INTEGER NOT NULL DEFAULT 0,
                state TEXT NOT NULL DEFAULT 'Active',
                start_time INTEGER NOT NULL DEFAULT 0,
                pause_time INTEGER
            );

            CREATE TABLE IF NOT EXISTS treasury (
                balance TEXT NOT NULL DEFAULT '0',
                proposals TEXT NOT NULL DEFAULT '{}'
            );

            CREATE TABLE IF NOT EXISTS blocks (
                height INTEGER PRIMARY KEY,
                hash BLOB NOT NULL,
                parent_hash BLOB NOT NULL,
                timestamp INTEGER NOT NULL DEFAULT 0,
                validator_id INTEGER NOT NULL DEFAULT 0,
                reward_amount TEXT NOT NULL DEFAULT '0',
                gas_fees TEXT NOT NULL DEFAULT '0',
                state_root BLOB NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chain_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )
        .context("failed to create tables")?;
        Ok(())
    }

    // ── Accounts ──

    pub fn upsert_account(&self, account: &Account) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO accounts (id, oikos_balance, koin_balance, nonce)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                account.id,
                account.oikos_balance.0.to_string(),
                account.koin_balance.0.to_string(),
                account.nonce,
            ],
        )?;
        Ok(())
    }

    pub fn get_account(&self, id: AccountId) -> Result<Option<Account>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, oikos_balance, koin_balance, nonce FROM accounts WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Account {
                id: row.get(0)?,
                oikos_balance: OikosAmount(row.get::<_, String>(1)?.parse().unwrap_or(0)),
                koin_balance: KoinAmount(row.get::<_, String>(2)?.parse().unwrap_or(0)),
                nonce: row.get(3)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, oikos_balance, koin_balance, nonce FROM accounts")?;
        let rows = stmt.query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                oikos_balance: OikosAmount(row.get::<_, String>(1)?.parse().unwrap_or(0)),
                koin_balance: KoinAmount(row.get::<_, String>(2)?.parse().unwrap_or(0)),
                nonce: row.get(3)?,
            })
        })?;
        let accounts = rows.filter_map(|r| r.ok()).collect();
        Ok(accounts)
    }

    // ── Validators ──

    pub fn upsert_validator(&self, v: &Validator) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO validators
             (id, operator_did, staked_amount, reward_debt, active, slashed_amount, created_at, jailed_until)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                v.id,
                v.operator_did,
                v.staked_amount.0.to_string(),
                v.reward_debt.to_string(),
                v.active as i32,
                v.slashed_amount.0.to_string(),
                v.created_at,
                v.jailed_until,
            ],
        )?;
        Ok(())
    }

    pub fn get_validator(&self, id: u64) -> Result<Option<Validator>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, operator_did, staked_amount, reward_debt, active, slashed_amount, created_at, jailed_until
             FROM validators WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Validator {
                id: row.get(0)?,
                operator_did: row.get(1)?,
                staked_amount: OikosAmount(row.get::<_, String>(2)?.parse().unwrap_or(0)),
                reward_debt: row.get::<_, String>(3)?.parse().unwrap_or(0),
                active: row.get::<_, i32>(4)? != 0,
                slashed_amount: OikosAmount(row.get::<_, String>(5)?.parse().unwrap_or(0)),
                created_at: row.get(6)?,
                jailed_until: row.get(7)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn list_validators(&self) -> Result<Vec<Validator>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, operator_did, staked_amount, reward_debt, active, slashed_amount, created_at, jailed_until
             FROM validators",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Validator {
                id: row.get(0)?,
                operator_did: row.get(1)?,
                staked_amount: OikosAmount(row.get::<_, String>(2)?.parse().unwrap_or(0)),
                reward_debt: row.get::<_, String>(3)?.parse().unwrap_or(0),
                active: row.get::<_, i32>(4)? != 0,
                slashed_amount: OikosAmount(row.get::<_, String>(5)?.parse().unwrap_or(0)),
                created_at: row.get(6)?,
                jailed_until: row.get(7)?,
            })
        })?;
        let validators = rows.filter_map(|r| r.ok()).collect();
        Ok(validators)
    }

    // ── Transactions ──

    pub fn insert_transaction(&self, tx: &Transaction) -> Result<()> {
        let kind_str = format!("{:?}", tx.kind);
        let parent_hashes_json = serde_json::to_string(
            &tx.parent_hashes.iter().map(|h| hex::encode(h)).collect::<Vec<_>>()
        ).unwrap_or_else(|_| "[]".to_string());
        let hash_data = tx.hash;
        let hash_ref: &[u8] = &hash_data;
        let affected = self.conn.execute(
            "INSERT OR REPLACE INTO transactions
             (hash, kind, sender, recipient, oikos_amount, koin_amount, gas_limit, nonce, parent_hashes, timestamp, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                hash_ref,
                kind_str,
                tx.sender,
                tx.recipient,
                tx.oikos_amount.0.to_string(),
                tx.koin_amount.0.to_string(),
                tx.gas_limit,
                tx.nonce,
                parent_hashes_json,
                tx.timestamp,
                "Pending",
            ],
        )?;
        anyhow::ensure!(affected > 0, "insert_transaction: 0 rows affected, hash={:?}", tx.hash);
        Ok(())
    }

    pub fn list_transactions(&self, limit: usize) -> Result<Vec<Transaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT hash, kind, sender, recipient, oikos_amount, koin_amount, gas_limit, nonce, parent_hashes, timestamp
             FROM transactions ORDER BY rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let hash_bytes: Vec<u8> = row.get(0)?;
            let kind_str: String = row.get(1)?;
            let parent_hashes_str: String = row.get(8)?;

            let mut hash = [0u8; 32];
            if hash_bytes.len() == 32 {
                hash.copy_from_slice(&hash_bytes);
            }

            let kind = parse_tx_kind(&kind_str);

            let parent_hashes_strs: Vec<String> = serde_json::from_str(&parent_hashes_str)
                .unwrap_or_default();
            let parent_hashes: Vec<TxHash> = parent_hashes_strs
                .iter()
                .filter_map(|s| {
                    let bytes = hex::decode(s).ok()?;
                    if bytes.len() == 32 {
                        let mut h = [0u8; 32];
                        h.copy_from_slice(&bytes);
                        Some(h)
                    } else {
                        None
                    }
                })
                .collect();

            Ok(Transaction {
                hash,
                kind,
                sender: row.get(2)?,
                recipient: row.get(3)?,
                oikos_amount: OikosAmount(row.get::<_, String>(4)?.parse().unwrap_or(0)),
                koin_amount: KoinAmount(row.get::<_, String>(5)?.parse().unwrap_or(0)),
                gas_limit: row.get(6)?,
                nonce: row.get(7)?,
                parent_hashes,
                timestamp: row.get(9)?,
            })
        })?;
        let txs = rows.filter_map(|r| r.ok()).collect();
        Ok(txs)
    }

    pub fn get_transaction(&self, hash: &TxHash) -> Result<Option<Transaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT hash, kind, sender, recipient, oikos_amount, koin_amount, gas_limit, nonce, parent_hashes, timestamp
             FROM transactions WHERE hash = ?1",
        )?;
        let mut rows = stmt.query_map(params![hash.as_slice()], |row| {
            let hash_bytes: Vec<u8> = row.get(0)?;
            let kind_str: String = row.get(1)?;
            let parent_hashes_str: String = row.get(8)?;

            let mut h = [0u8; 32];
            if hash_bytes.len() == 32 {
                h.copy_from_slice(&hash_bytes);
            }

            let kind = parse_tx_kind(&kind_str);

            let parent_hashes_strs: Vec<String> = serde_json::from_str(&parent_hashes_str)
                .unwrap_or_default();
            let parent_hashes: Vec<TxHash> = parent_hashes_strs
                .iter()
                .filter_map(|s| {
                    let bytes = hex::decode(s).ok()?;
                    if bytes.len() == 32 {
                        let mut ph = [0u8; 32];
                        ph.copy_from_slice(&bytes);
                        Some(ph)
                    } else {
                        None
                    }
                })
                .collect();

            Ok(Transaction {
                hash: h,
                kind,
                sender: row.get(2)?,
                recipient: row.get(3)?,
                oikos_amount: OikosAmount(row.get::<_, String>(4)?.parse().unwrap_or(0)),
                koin_amount: KoinAmount(row.get::<_, String>(5)?.parse().unwrap_or(0)),
                gas_limit: row.get(6)?,
                nonce: row.get(7)?,
                parent_hashes,
                timestamp: row.get(9)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    // ── Mandates ──

    pub fn upsert_mandate(&self, m: &AgentMandate) -> Result<()> {
        let scopes_json = serde_json::to_string(
            &m.scopes.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>()
        ).unwrap_or_else(|_| "[]".to_string());
        self.conn.execute(
            "INSERT OR REPLACE INTO mandates
             (id, principal_did, agent_did, oikos_budget, koin_budget, scopes, time_bound, active, oikos_spent, koin_spent)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                m.id,
                m.principal_did,
                m.agent_did,
                m.oikos_budget.0.to_string(),
                m.koin_budget.0.to_string(),
                scopes_json,
                m.time_bound,
                m.active as i32,
                m.oikos_spent.0.to_string(),
                m.koin_spent.0.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn get_mandate(&self, id: u64) -> Result<Option<AgentMandate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, principal_did, agent_did, oikos_budget, koin_budget, scopes, time_bound, active, oikos_spent, koin_spent
             FROM mandates WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            let scopes_strs: Vec<String> = serde_json::from_str(
                &row.get::<_, String>(5)?
            ).unwrap_or_default();
            let scopes: Vec<MandateScope> = scopes_strs.iter().map(|s| parse_scope(s)).collect();
            Ok(AgentMandate {
                id: row.get(0)?,
                did: String::new(),
                principal_did: row.get(1)?,
                agent_did: row.get(2)?,
                oikos_budget: OikosAmount(row.get::<_, String>(3)?.parse().unwrap_or(0)),
                koin_budget: KoinAmount(row.get::<_, String>(4)?.parse().unwrap_or(0)),
                scopes,
                allowed_contracts: Vec::new(),
                time_bound: row.get(6)?,
                active: row.get::<_, i32>(7)? != 0,
                oikos_spent: OikosAmount(row.get::<_, String>(8)?.parse().unwrap_or(0)),
                koin_spent: KoinAmount(row.get::<_, String>(9)?.parse().unwrap_or(0)),
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn list_mandates(&self) -> Result<Vec<AgentMandate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, principal_did, agent_did, oikos_budget, koin_budget, scopes, time_bound, active, oikos_spent, koin_spent
             FROM mandates",
        )?;
        let rows = stmt.query_map([], |row| {
            let scopes_strs: Vec<String> = serde_json::from_str(
                &row.get::<_, String>(5)?
            ).unwrap_or_default();
            let scopes: Vec<MandateScope> = scopes_strs.iter().map(|s| parse_scope(s)).collect();
            Ok(AgentMandate {
                id: row.get(0)?,
                did: String::new(),
                principal_did: row.get(1)?,
                agent_did: row.get(2)?,
                oikos_budget: OikosAmount(row.get::<_, String>(3)?.parse().unwrap_or(0)),
                koin_budget: KoinAmount(row.get::<_, String>(4)?.parse().unwrap_or(0)),
                scopes,
                allowed_contracts: Vec::new(),
                time_bound: row.get(6)?,
                active: row.get::<_, i32>(7)? != 0,
                oikos_spent: OikosAmount(row.get::<_, String>(8)?.parse().unwrap_or(0)),
                koin_spent: KoinAmount(row.get::<_, String>(9)?.parse().unwrap_or(0)),
            })
        })?;
        let mandates = rows.filter_map(|r| r.ok()).collect();
        Ok(mandates)
    }

    // ── Blocks ──

    pub fn apply_block(&self, block: &Block) -> Result<()> {
        let gas_fees = block.reward.fee_burn.0 + block.reward.fee_validator.0 + block.reward.fee_treasury.0;
        self.conn.execute(
            "INSERT OR REPLACE INTO blocks
             (height, hash, parent_hash, timestamp, validator_id, reward_amount, gas_fees, state_root)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                block.height,
                block.hash.as_slice(),
                block.parent_hash.as_slice(),
                block.timestamp,
                block.validator_id,
                block.reward.base_reward.0.to_string(),
                gas_fees.to_string(),
                block.state_root.as_slice(),
            ],
        )?;
        self.set_chain_state("last_block_height", &block.height.to_string())?;
        Ok(())
    }

    pub fn get_block(&self, height: u64) -> Result<Option<Block>> {
        let mut stmt = self.conn.prepare(
            "SELECT height, hash, parent_hash, timestamp, validator_id, reward_amount, gas_fees, state_root
             FROM blocks WHERE height = ?1",
        )?;
        let mut rows = stmt.query_map(params![height], |row| {
            let hash_bytes: Vec<u8> = row.get(1)?;
            let parent_bytes: Vec<u8> = row.get(2)?;
            let state_bytes: Vec<u8> = row.get(7)?;

            let mut hash = [0u8; 32];
            if hash_bytes.len() == 32 { hash.copy_from_slice(&hash_bytes); }
            let mut parent_hash = [0u8; 32];
            if parent_bytes.len() == 32 { parent_hash.copy_from_slice(&parent_bytes); }
            let mut state_root = [0u8; 32];
            if state_bytes.len() == 32 { state_root.copy_from_slice(&state_bytes); }

            let reward_amount: u128 = row.get::<_, String>(5)?.parse().unwrap_or(0);

            Ok(Block {
                height: row.get(0)?,
                hash,
                parent_hash,
                timestamp: row.get(3)?,
                transactions: Vec::new(),
                validator_id: row.get(4)?,
                reward: koinon_rewards::BlockReward {
                    block_number: 0,
                    year: 0,
                    base_reward: OikosAmount(reward_amount),
                    fee_burn: KoinAmount::ZERO,
                    fee_validator: KoinAmount::ZERO,
                    fee_treasury: KoinAmount::ZERO,
                },
                state_root,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    // ── Chain State ──

    pub fn get_last_block_height(&self) -> Result<u64> {
        self.get_chain_state("last_block_height")
            .map(|v| v.and_then(|s| s.parse().ok()).unwrap_or(0))
    }

    pub fn set_chain_state(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO chain_state (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_chain_state(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM chain_state WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }
}

fn parse_scope(s: &str) -> MandateScope {
    if s.contains("TransferOikos") { MandateScope::TransferOikos }
    else if s.contains("TransferKoin") { MandateScope::TransferKoin }
    else if s.contains("MintKoin") { MandateScope::MintKoin }
    else if s.contains("BurnKoin") { MandateScope::BurnKoin }
    else if s.contains("EscrowManage") { MandateScope::EscrowManage }
    else if s.contains("StreamManage") { MandateScope::StreamManage }
    else if s.contains("RfpPublish") { MandateScope::RfpPublish }
    else if s.contains("RfpRespond") { MandateScope::RfpRespond }
    else if let Some(did) = s.strip_prefix("DelegateTo(") {
        MandateScope::DelegateTo(did.trim_end_matches(')').to_string())
    }
    else if let Some(name) = s.strip_prefix("Custom(") {
        MandateScope::Custom(name.trim_end_matches(')').to_string())
    }
    else {
        MandateScope::TransferOikos
    }
}

fn parse_tx_kind(s: &str) -> TxKind {
    if s.contains("TransferOikos") { TxKind::TransferOikos }
    else if s.contains("TransferKoin") { TxKind::TransferKoin }
    else if s.contains("MintKoin") { TxKind::MintKoin }
    else if s.contains("BurnKoin") { TxKind::BurnKoin }
    else if s.contains("EscrowCreate") { TxKind::EscrowCreate }
    else if s.contains("EscrowRelease") { TxKind::EscrowRelease }
    else if s.contains("EscrowCancel") { TxKind::EscrowCancel }
    else if s.contains("StreamStart") { TxKind::StreamStart }
    else if s.contains("StreamStop") { TxKind::StreamStop }
    else if s.contains("RfpPublish") { TxKind::RfpPublish }
    else if s.contains("RfpRespond") { TxKind::RfpRespond }
    else if s.contains("MandateCreate") { TxKind::MandateCreate }
    else { TxKind::MandateSpend }
}

/// Simple hex helper module (inline to avoid extra dependency).
mod hex {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";

    pub fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        s
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, ()> {
        if s.len() % 2 != 0 {
            return Err(());
        }
        let mut bytes = Vec::with_capacity(s.len() / 2);
        for chunk in s.as_bytes().chunks(2) {
            let hi = from_hex_char(chunk[0])?;
            let lo = from_hex_char(chunk[1])?;
            bytes.push((hi << 4) | lo);
        }
        Ok(bytes)
    }

    fn from_hex_char(c: u8) -> Result<u8, ()> {
        match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use koinon_rewards::BlockRewardConfig;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_store() -> StateStore {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("koinon_node_test_{}_{}", std::process::id(), id));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.db");
        let _ = std::fs::remove_file(&path);
        StateStore::new(path.to_str().unwrap()).unwrap()
    }

    #[test]
    fn open_and_create_tables() {
        let _store = temp_store();
    }

    #[test]
    fn upsert_and_get_account() {
        let store = temp_store();
        let account = Account::new(42);
        store.upsert_account(&account).unwrap();
        let loaded = store.get_account(42).unwrap().unwrap();
        assert_eq!(loaded.id, 42);
        assert_eq!(loaded.oikos_balance, OikosAmount::ZERO);
    }

    #[test]
    fn get_nonexistent_account() {
        let store = temp_store();
        assert!(store.get_account(999).unwrap().is_none());
    }

    #[test]
    fn list_accounts() {
        let store = temp_store();
        store.upsert_account(&Account::new(1)).unwrap();
        store.upsert_account(&Account::new(2)).unwrap();
        let accounts = store.list_accounts().unwrap();
        assert_eq!(accounts.len(), 2);
    }

    #[test]
    fn upsert_and_get_validator() {
        let store = temp_store();
        let mut pool = koinon_staking::staking::StakingPool::new();
        let id = pool.register_validator("did:example:op1").unwrap();
        let v = pool.get_validator(id).unwrap();
        store.upsert_validator(v).unwrap();
        let loaded = store.get_validator(id).unwrap().unwrap();
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.operator_did, "did:example:op1");
    }

    #[test]
    fn insert_and_list_transactions() {
        let store = temp_store();
        let tx = Transaction {
            hash: [2u8; 32],
            kind: TxKind::TransferKoin,
            sender: 1,
            recipient: 2,
            oikos_amount: OikosAmount::ZERO,
            koin_amount: KoinAmount(100),
            gas_limit: 21000,
            nonce: 0,
            parent_hashes: vec![],
            timestamp: 1000,
        };
        store.insert_transaction(&tx).unwrap();
        let txs = store.list_transactions(10).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].hash, [2u8; 32]);
    }

    #[test]
    fn apply_block_and_get_block() {
        let store = temp_store();
        let mut proc = koinon_rewards::BlockRewardProcessor::new(BlockRewardConfig::default());
        let reward = proc.process_block(1, KoinAmount::ZERO).unwrap();
        let block = Block {
            height: 1,
            hash: [1u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 1000,
            transactions: Vec::new(),
            validator_id: 1,
            reward,
            state_root: [2u8; 32],
        };
        store.apply_block(&block).unwrap();
        let loaded = store.get_block(1).unwrap().unwrap();
        assert_eq!(loaded.height, 1);
        assert_eq!(loaded.hash, [1u8; 32]);
        assert_eq!(store.get_last_block_height().unwrap(), 1);
    }

    #[test]
    fn chain_state_roundtrip() {
        let store = temp_store();
        store.set_chain_state("test_key", "test_value").unwrap();
        let val = store.get_chain_state("test_key").unwrap();
        assert_eq!(val.as_deref(), Some("test_value"));
    }

    #[test]
    fn upsert_and_get_mandate() {
        let store = temp_store();
        let config = koinon_mandates::mandate::MandateConfig {
            did: "did:example:mandate1".to_string(),
            principal_did: "did:example:alice".to_string(),
            agent_did: "did:example:bob".to_string(),
            oikos_budget: OikosAmount(1000),
            koin_budget: KoinAmount(500),
            scopes: vec![MandateScope::TransferOikos],
            allowed_contracts: vec![],
            time_bound: Some(9999),
        };
        let mandate = AgentMandate::create(1, config);
        store.upsert_mandate(&mandate).unwrap();
        let loaded = store.get_mandate(1).unwrap().unwrap();
        assert_eq!(loaded.id, 1);
        assert_eq!(loaded.principal_did, "did:example:alice");
        assert!(loaded.active);
    }

    #[test]
    fn hex_encode_decode_roundtrip() {
        let data = vec![0xde, 0xad, 0xbe, 0xef, 0x42, 0x01];
        let encoded = hex::encode(&data);
        assert_eq!(encoded, "deadbeef4201");
        let decoded = hex::decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}
