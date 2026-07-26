use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use koinon_ledger::{OikosAmount, KoinAmount};

use crate::block::BlockProducer;

/// Shared state for the RPC server.
pub type SharedProducer = Arc<Mutex<BlockProducer>>;

/// JSON-RPC request.
#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub method: String,
    pub params: serde_json::Value,
    pub id: Option<u64>,
}

/// JSON-RPC response.
#[derive(Debug, Serialize)]
pub struct RpcResponse {
    pub result: serde_json::Value,
    pub error: Option<String>,
    pub id: Option<u64>,
}

impl RpcResponse {
    fn ok(result: serde_json::Value, id: Option<u64>) -> Self {
        Self { result, error: None, id }
    }

    fn err(msg: &str, id: Option<u64>) -> Self {
        Self {
            result: serde_json::Value::Null,
            error: Some(msg.to_string()),
            id,
        }
    }
}

/// Start the RPC server on the given port.
pub fn start_rpc_server(port: u16, producer: SharedProducer) -> Result<()> {
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr)?;
    log::info!("RPC server listening on {addr}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let producer = Arc::clone(&producer);
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, producer) {
                        log::error!("connection error: {e}");
                    }
                });
            }
            Err(e) => {
                log::error!("failed to accept connection: {e}");
            }
        }
    }
    Ok(())
}

fn handle_connection(stream: TcpStream, producer: SharedProducer) -> Result<()> {
    let reader_stream = stream.try_clone()?;
    let mut writer = stream;
    let reader = BufReader::new(reader_stream);

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(req) => handle_request(&req, &producer),
            Err(e) => RpcResponse::err(&format!("invalid request: {e}"), None),
        };

        let resp_json = serde_json::to_string(&response)?;
        writer.write_all(resp_json.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    Ok(())
}

fn handle_request(req: &RpcRequest, producer: &SharedProducer) -> RpcResponse {
    let id = req.id;
    match req.method.as_str() {
        "health" => RpcResponse::ok(serde_json::json!({"status": "ok"}), id),
        "status" => handle_status(producer, id),
        "block" => handle_get_block(req, producer, id),
        "tx" => handle_get_tx(req, producer, id),
        "submit_tx" => handle_submit_tx(req, producer, id),
        "accounts" => handle_get_account(req, producer, id),
        "validators" => handle_list_validators(producer, id),
        "validator" => handle_get_validator(req, producer, id),
        "mandates" => handle_list_mandates(producer, id),
        "treasury" => handle_treasury(producer, id),
        "emission" => handle_emission(id),
        _ => RpcResponse::err(&format!("unknown method: {}", req.method), id),
    }
}

fn handle_status(producer: &SharedProducer, id: Option<u64>) -> RpcResponse {
    let producer = producer.lock().unwrap();
    let state = producer.get_state();
    RpcResponse::ok(
        serde_json::json!({
            "year": state.reward_processor.current_year(),
            "total_staked": state.staking_pool.total_staked().0.to_string(),
            "mempool_size": producer.mempool_size(),
            "conservation": {
                "minted": state.conservation.minted.0.to_string(),
                "burned": state.conservation.burned.0.to_string(),
            }
        }),
        id,
    )
}

fn handle_get_block(req: &RpcRequest, producer: &SharedProducer, id: Option<u64>) -> RpcResponse {
    let _height = match req.params.get("height").and_then(|v| v.as_u64()) {
        Some(h) => h,
        None => return RpcResponse::err("missing 'height' parameter", id),
    };
    let producer = producer.lock().unwrap();
    let _state = producer.get_state();
    RpcResponse::ok(
        serde_json::json!({
            "message": "block retrieval requires store access"
        }),
        id,
    )
}

fn handle_get_tx(req: &RpcRequest, _producer: &SharedProducer, id: Option<u64>) -> RpcResponse {
    let hash_str = match req.params.get("hash").and_then(|v| v.as_str()) {
        Some(h) => h,
        None => return RpcResponse::err("missing 'hash' parameter", id),
    };
    RpcResponse::ok(
        serde_json::json!({
            "hash": hash_str,
            "message": "transaction lookup requires store access"
        }),
        id,
    )
}

fn handle_submit_tx(req: &RpcRequest, producer: &SharedProducer, id: Option<u64>) -> RpcResponse {
    let mut producer = producer.lock().unwrap();
    let tx_data = match req.params.get("transaction") {
        Some(d) => d,
        None => return RpcResponse::err("missing 'transaction' parameter", id),
    };

    let hash: [u8; 32] = match tx_data.get("hash").and_then(|v| v.as_str()) {
        Some(s) => {
            let bytes = hex::decode(s).unwrap_or_default();
            if bytes.len() != 32 {
                return RpcResponse::err("invalid hash length", id);
            }
            let mut h = [0u8; 32];
            h.copy_from_slice(&bytes);
            h
        }
        None => return RpcResponse::err("missing transaction hash", id),
    };

    let tx = koinon_dag::tx::Transaction {
        hash,
        kind: koinon_dag::tx::TxKind::TransferKoin,
        sender: tx_data.get("sender").and_then(|v| v.as_u64()).unwrap_or(0),
        recipient: tx_data.get("recipient").and_then(|v| v.as_u64()).unwrap_or(0),
        oikos_amount: OikosAmount(tx_data.get("oikos_amount").and_then(|v| v.as_u64()).unwrap_or(0) as u128),
        koin_amount: KoinAmount(tx_data.get("koin_amount").and_then(|v| v.as_i64()).unwrap_or(0) as i128),
        gas_limit: tx_data.get("gas_limit").and_then(|v| v.as_u64()).unwrap_or(21000),
        nonce: tx_data.get("nonce").and_then(|v| v.as_u64()).unwrap_or(0),
        parent_hashes: vec![],
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    match producer.submit_transaction(tx) {
        Ok(hash) => RpcResponse::ok(
            serde_json::json!({"tx_hash": hex::encode(&hash)}),
            id,
        ),
        Err(e) => RpcResponse::err(&e.to_string(), id),
    }
}

fn handle_get_account(req: &RpcRequest, producer: &SharedProducer, id: Option<u64>) -> RpcResponse {
    let account_id = match req.params.get("id").and_then(|v| v.as_u64()) {
        Some(a) => a,
        None => return RpcResponse::err("missing 'id' parameter", id),
    };
    let producer = producer.lock().unwrap();
    let state = producer.get_state();
    match state.accounts.get(&account_id) {
        Some(acc) => RpcResponse::ok(
            serde_json::json!({
                "id": acc.id,
                "oikos_balance": acc.oikos_balance.0,
                "koin_balance": acc.koin_balance.0,
                "nonce": acc.nonce,
            }),
            id,
        ),
        None => RpcResponse::err(&format!("account {account_id} not found"), id),
    }
}

fn handle_list_validators(producer: &SharedProducer, id: Option<u64>) -> RpcResponse {
    let producer = producer.lock().unwrap();
    let state = producer.get_state();
    let validators: Vec<_> = state.staking_pool.validators.values().map(|v| {
        serde_json::json!({
            "id": v.id,
            "operator_did": v.operator_did,
            "staked_amount": v.staked_amount.0,
            "active": v.active,
            "jailed_until": v.jailed_until,
        })
    }).collect();
    RpcResponse::ok(serde_json::json!(validators), id)
}

fn handle_get_validator(req: &RpcRequest, producer: &SharedProducer, id: Option<u64>) -> RpcResponse {
    let validator_id = match req.params.get("id").and_then(|v| v.as_u64()) {
        Some(v) => v,
        None => return RpcResponse::err("missing 'id' parameter", id),
    };
    let producer = producer.lock().unwrap();
    let state = producer.get_state();
    match state.staking_pool.get_validator(validator_id) {
        Some(v) => RpcResponse::ok(
            serde_json::json!({
                "id": v.id,
                "operator_did": v.operator_did,
                "staked_amount": v.staked_amount.0,
                "reward_debt": v.reward_debt,
                "active": v.active,
                "slashed_amount": v.slashed_amount.0,
                "created_at": v.created_at,
                "jailed_until": v.jailed_until,
            }),
            id,
        ),
        None => RpcResponse::err(&format!("validator {validator_id} not found"), id),
    }
}

fn handle_list_mandates(producer: &SharedProducer, id: Option<u64>) -> RpcResponse {
    let producer = producer.lock().unwrap();
    let state = producer.get_state();
    let mandates: Vec<_> = state.mandates.iter().map(|m| {
        serde_json::json!({
            "id": m.id,
            "principal_did": m.principal_did,
            "agent_did": m.agent_did,
            "oikos_budget": m.oikos_budget.0,
            "koin_budget": m.koin_budget.0,
            "active": m.active,
        })
    }).collect();
    RpcResponse::ok(serde_json::json!(mandates), id)
}

fn handle_treasury(producer: &SharedProducer, id: Option<u64>) -> RpcResponse {
    let producer = producer.lock().unwrap();
    let state = producer.get_state();
    RpcResponse::ok(
        serde_json::json!({
            "balance": state.treasury.balance.0,
            "proposal_count": state.treasury.proposals.len(),
        }),
        id,
    )
}

fn handle_emission(id: Option<u64>) -> RpcResponse {
    let schedule: Vec<_> = koinon_ledger::emission_schedule().into_iter().map(|e| {
        serde_json::json!({
            "year": e.year,
            "annual_emission": e.annual_emission,
            "cumulative_supply": e.cumulative_supply,
        })
    }).collect();
    RpcResponse::ok(serde_json::json!(schedule), id)
}

/// Simple hex helper (no external dep).
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
    use crate::store::StateStore;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::sync::Mutex;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn shared_producer() -> SharedProducer {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "koinon_node_api_test_{}_{}",
            std::process::id(), id
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_api.db");
        let _ = std::fs::remove_file(&path);
        let store = StateStore::new(path.to_str().unwrap()).unwrap();
        let config = crate::config::NodeConfig::default();
        let producer = crate::block::BlockProducer::new(config, store).unwrap();
        Arc::new(Mutex::new(producer))
    }

    #[test]
    fn health_endpoint() {
        let producer = shared_producer();
        let req = RpcRequest {
            method: "health".to_string(),
            params: serde_json::json!({}),
            id: Some(1),
        };
        let resp = handle_request(&req, &producer);
        assert!(resp.error.is_none());
        assert_eq!(resp.result["status"], "ok");
        assert_eq!(resp.id, Some(1));
    }

    #[test]
    fn status_endpoint() {
        let producer = shared_producer();
        let req = RpcRequest {
            method: "status".to_string(),
            params: serde_json::json!({}),
            id: Some(2),
        };
        let resp = handle_request(&req, &producer);
        assert!(resp.error.is_none());
        assert!(resp.result.get("total_staked").is_some());
    }

    #[test]
    fn unknown_method_returns_error() {
        let producer = shared_producer();
        let req = RpcRequest {
            method: "nonexistent".to_string(),
            params: serde_json::json!({}),
            id: Some(3),
        };
        let resp = handle_request(&req, &producer);
        assert!(resp.error.is_some());
    }

    #[test]
    fn get_account_missing() {
        let producer = shared_producer();
        let req = RpcRequest {
            method: "accounts".to_string(),
            params: serde_json::json!({"id": 999}),
            id: Some(4),
        };
        let resp = handle_request(&req, &producer);
        assert!(resp.error.is_some());
    }

    #[test]
    fn emission_endpoint() {
        let producer = shared_producer();
        let req = RpcRequest {
            method: "emission".to_string(),
            params: serde_json::json!({}),
            id: Some(5),
        };
        let resp = handle_request(&req, &producer);
        assert!(resp.error.is_none());
        assert!(resp.result.is_array());
    }
}
