use koinon_ledger::{AccountId, OikosAmount, KoinAmount};

pub type TxHash = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TxKind {
    TransferOikos,
    TransferKoin,
    MintKoin,
    BurnKoin,
    EscrowCreate,
    EscrowRelease,
    EscrowCancel,
    StreamStart,
    StreamStop,
    RfpPublish,
    RfpRespond,
    MandateCreate,
    MandateSpend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxStatus {
    Pending,
    Settled,
    Failed,
    Conflict,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub hash: TxHash,
    pub kind: TxKind,
    pub sender: AccountId,
    pub recipient: AccountId,
    pub oikos_amount: OikosAmount,
    pub koin_amount: KoinAmount,
    pub gas_limit: u64,
    pub nonce: u64,
    pub parent_hashes: Vec<TxHash>,
    pub timestamp: u64,
}

impl Transaction {
    pub fn id(&self) -> TxHash {
        self.hash
    }

    pub fn conflicts_with(&self, other: &Transaction) -> bool {
        self.sender == other.sender && self.nonce == other.nonce
    }

    pub fn dependencies(&self) -> &[TxHash] {
        &self.parent_hashes
    }
}
