use serde::{Deserialize, Serialize};

pub type PeerId = [u8; 32];
pub type TxHash = [u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHeader {
    pub height: u64,
    pub hash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub timestamp: u64,
    pub validator_id: u64,
    pub tx_count: u32,
    pub state_root: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<WireTx>,
}

/// Wire-compatible representation of a transaction.
///
/// Uses primitive types for serde compatibility. Convert to/from
/// `koinon_dag::Transaction` via the `From`/`TryFrom` implementations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WireTx {
    pub hash: [u8; 32],
    pub kind: u8,
    pub sender: u64,
    pub recipient: u64,
    pub oikos_amount: u128,
    pub koin_amount: i128,
    pub gas_limit: u64,
    pub nonce: u64,
    pub parent_hashes: Vec<[u8; 32]>,
    pub timestamp: u64,
}

impl From<koinon_dag::Transaction> for WireTx {
    fn from(tx: koinon_dag::Transaction) -> Self {
        use koinon_dag::TxKind;
        let kind = match tx.kind {
            TxKind::TransferOikos => 0u8,
            TxKind::TransferKoin => 1,
            TxKind::MintKoin => 2,
            TxKind::BurnKoin => 3,
            TxKind::EscrowCreate => 4,
            TxKind::EscrowRelease => 5,
            TxKind::EscrowCancel => 6,
            TxKind::StreamStart => 7,
            TxKind::StreamStop => 8,
            TxKind::RfpPublish => 9,
            TxKind::RfpRespond => 10,
            TxKind::MandateCreate => 11,
            TxKind::MandateSpend => 12,
        };
        Self {
            hash: tx.hash,
            kind,
            sender: tx.sender,
            recipient: tx.recipient,
            oikos_amount: tx.oikos_amount.0,
            koin_amount: tx.koin_amount.0,
            gas_limit: tx.gas_limit,
            nonce: tx.nonce,
            parent_hashes: tx.parent_hashes,
            timestamp: tx.timestamp,
        }
    }
}

impl TryFrom<WireTx> for koinon_dag::Transaction {
    type Error = &'static str;

    fn try_from(wtx: WireTx) -> Result<Self, Self::Error> {
        use koinon_dag::TxKind;
        let kind = match wtx.kind {
            0 => TxKind::TransferOikos,
            1 => TxKind::TransferKoin,
            2 => TxKind::MintKoin,
            3 => TxKind::BurnKoin,
            4 => TxKind::EscrowCreate,
            5 => TxKind::EscrowRelease,
            6 => TxKind::EscrowCancel,
            7 => TxKind::StreamStart,
            8 => TxKind::StreamStop,
            9 => TxKind::RfpPublish,
            10 => TxKind::RfpRespond,
            11 => TxKind::MintKoin,
            12 => TxKind::MandateSpend,
            _ => return Err("invalid tx kind discriminator"),
        };
        Ok(koinon_dag::Transaction {
            hash: wtx.hash,
            kind,
            sender: wtx.sender,
            recipient: wtx.recipient,
            oikos_amount: koinon_ledger::OikosAmount(wtx.oikos_amount),
            koin_amount: koinon_ledger::KoinAmount(wtx.koin_amount),
            gas_limit: wtx.gas_limit,
            nonce: wtx.nonce,
            parent_hashes: wtx.parent_hashes,
            timestamp: wtx.timestamp,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    // Block propagation
    NewBlock(BlockHeader),
    BlockRequest(u64),
    BlockResponse(Block),

    // Transaction propagation
    NewTransaction(WireTx),
    TransactionRequest(TxHash),
    TransactionResponse(WireTx),

    // State sync
    StateSyncRequest {
        from_height: u64,
        to_height: u64,
    },
    StateSyncResponse(Vec<Block>),

    // Validator discovery
    Ping {
        node_id: PeerId,
        height: u64,
    },
    Pong {
        node_id: PeerId,
        height: u64,
    },

    // Handshake
    Handshake {
        chain_id: u64,
        node_id: PeerId,
        version: u32,
    },
    HandshakeAck {
        chain_id: u64,
        node_id: PeerId,
        best_height: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_header(height: u64) -> BlockHeader {
        BlockHeader {
            height,
            hash: [1u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 1_700_000_000,
            validator_id: 42,
            tx_count: 5,
            state_root: [2u8; 32],
        }
    }

    fn dummy_wire_tx() -> WireTx {
        WireTx {
            hash: [3u8; 32],
            kind: 0,
            sender: 1,
            recipient: 2,
            oikos_amount: 1_000_000_000_000_000_000,
            koin_amount: 500,
            gas_limit: 21_000,
            nonce: 1,
            parent_hashes: vec![[4u8; 32]],
            timestamp: 1_700_000_001,
        }
    }

    #[test]
    fn block_header_round_trip() {
        let header = dummy_header(100);
        let json = serde_json::to_string(&header).unwrap();
        let decoded: BlockHeader = serde_json::from_str(&json).unwrap();
        assert_eq!(header, decoded);
    }

    #[test]
    fn block_round_trip() {
        let block = Block {
            header: dummy_header(1),
            transactions: vec![dummy_wire_tx()],
        };
        let json = serde_json::to_string(&block).unwrap();
        let decoded: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(block.header, decoded.header);
        assert_eq!(block.transactions.len(), 1);
    }

    #[test]
    fn wire_tx_round_trip() {
        let tx = dummy_wire_tx();
        let json = serde_json::to_string(&tx).unwrap();
        let decoded: WireTx = serde_json::from_str(&json).unwrap();
        assert_eq!(tx, decoded);
    }

    #[test]
    fn network_message_round_trip() {
        let messages = vec![
            NetworkMessage::NewBlock(dummy_header(1)),
            NetworkMessage::BlockRequest(42),
            NetworkMessage::BlockResponse(Block {
                header: dummy_header(2),
                transactions: vec![],
            }),
            NetworkMessage::NewTransaction(dummy_wire_tx()),
            NetworkMessage::TransactionRequest([5u8; 32]),
            NetworkMessage::TransactionResponse(dummy_wire_tx()),
            NetworkMessage::StateSyncRequest {
                from_height: 10,
                to_height: 20,
            },
            NetworkMessage::StateSyncResponse(vec![]),
            NetworkMessage::Ping {
                node_id: [6u8; 32],
                height: 100,
            },
            NetworkMessage::Pong {
                node_id: [7u8; 32],
                height: 200,
            },
            NetworkMessage::Handshake {
                chain_id: 1,
                node_id: [8u8; 32],
                version: 1,
            },
            NetworkMessage::HandshakeAck {
                chain_id: 1,
                node_id: [9u8; 32],
                best_height: 500,
            },
        ];

        for msg in &messages {
            let json = serde_json::to_string(msg).unwrap();
            let decoded: NetworkMessage = serde_json::from_str(&json).unwrap();
            let reserialized = serde_json::to_string(&decoded).unwrap();
            assert_eq!(json, reserialized);
        }
    }

    #[test]
    fn wire_tx_from_dag_transaction_and_back() {
        let dag_tx = koinon_dag::Transaction {
            hash: [10u8; 32],
            kind: koinon_dag::TxKind::TransferOikos,
            sender: 100,
            recipient: 200,
            oikos_amount: koinon_ledger::OikosAmount(42),
            koin_amount: koinon_ledger::KoinAmount(-7),
            gas_limit: 30_000,
            nonce: 5,
            parent_hashes: vec![[11u8; 32]],
            timestamp: 1_700_000_010,
        };

        let wire: WireTx = dag_tx.clone().into();
        let recovered: koinon_dag::Transaction = wire.try_into().unwrap();

        assert_eq!(dag_tx.hash, recovered.hash);
        assert_eq!(dag_tx.sender, recovered.sender);
        assert_eq!(dag_tx.recipient, recovered.recipient);
        assert_eq!(dag_tx.oikos_amount.0, recovered.oikos_amount.0);
        assert_eq!(dag_tx.koin_amount.0, recovered.koin_amount.0);
        assert_eq!(dag_tx.gas_limit, recovered.gas_limit);
        assert_eq!(dag_tx.nonce, recovered.nonce);
        assert_eq!(dag_tx.parent_hashes, recovered.parent_hashes);
        assert_eq!(dag_tx.timestamp, recovered.timestamp);
    }

    #[test]
    fn wire_tx_invalid_kind_fails_conversion() {
        let wtx = WireTx {
            hash: [0u8; 32],
            kind: 99,
            sender: 0,
            recipient: 0,
            oikos_amount: 0,
            koin_amount: 0,
            gas_limit: 0,
            nonce: 0,
            parent_hashes: vec![],
            timestamp: 0,
        };
        let result: Result<koinon_dag::Transaction, _> = wtx.try_into();
        assert!(result.is_err());
    }
}
