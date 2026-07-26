use anyhow::{Context, Result};
use serde::Deserialize;

use koinon_ledger::OikosAmount;

/// Node configuration loaded from TOML or defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct NodeConfig {
    pub data_dir: String,
    pub rpc_port: u16,
    pub block_time_ms: u64,
    pub genesis: GenesisConfig,
    pub log_level: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            data_dir: "./data".to_string(),
            rpc_port: 8545,
            block_time_ms: 1000,
            genesis: GenesisConfig::default(),
            log_level: "info".to_string(),
        }
    }
}

impl NodeConfig {
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {path}"))?;
        let config: NodeConfig = toml::from_str(&content)
            .with_context(|| format!("failed to parse config file: {path}"))?;
        Ok(config)
    }

    pub fn database_path(&self) -> String {
        format!("{}/chain.db", self.data_dir)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenesisConfig {
    pub initial_validators: Vec<GenesisValidator>,
    pub initial_accounts: Vec<GenesisAccount>,
    #[serde(deserialize_with = "deserialize_u128_string")]
    pub treasury_balance: u128,
    pub chain_id: u64,
}

fn deserialize_u128_string<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct U128Visitor;

    impl<'de> de::Visitor<'de> for U128Visitor {
        type Value = u128;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a u128 integer or string")
        }

        fn visit_u64<E>(self, v: u64) -> Result<u128, E> {
            Ok(v as u128)
        }

        fn visit_i64<E>(self, v: i64) -> Result<u128, E> {
            Ok(v as u128)
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<u128, E> {
            v.parse::<u128>().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(U128Visitor)
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            initial_validators: vec![GenesisValidator {
                operator_did: "did:example:genesis-validator".to_string(),
                stake: 100_000 * 10_u128.pow(18),
            }],
            initial_accounts: vec![
                GenesisAccount {
                    id: 1,
                    oikos_balance: 0,
                    koin_balance: 0,
                },
                GenesisAccount {
                    id: 2,
                    oikos_balance: 0,
                    koin_balance: 0,
                },
            ],
            treasury_balance: 1_000_000 * 10_u128.pow(18),
            chain_id: 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenesisValidator {
    pub operator_did: String,
    #[serde(deserialize_with = "deserialize_u128_string")]
    pub stake: u128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenesisAccount {
    pub id: u64,
    #[serde(deserialize_with = "deserialize_u128_string")]
    pub oikos_balance: u128,
    #[serde(default)]
    pub koin_balance: i64,
}

impl GenesisAccount {
    pub fn to_account(&self) -> koinon_ledger::Account {
        koinon_ledger::Account {
            id: self.id,
            oikos_balance: OikosAmount(self.oikos_balance),
            koin_balance: koinon_ledger::KoinAmount(self.koin_balance as i128),
            nonce: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = NodeConfig::default();
        assert_eq!(config.rpc_port, 8545);
        assert_eq!(config.block_time_ms, 1000);
        assert!(!config.data_dir.is_empty());
        assert!(!config.genesis.initial_validators.is_empty());
    }

    #[test]
    fn default_genesis_has_treasury() {
        let genesis = GenesisConfig::default();
        assert!(genesis.treasury_balance > 0);
        assert_eq!(genesis.chain_id, 1);
    }

    #[test]
    fn genesis_account_converts() {
        let ga = GenesisAccount {
            id: 42,
            oikos_balance: 1000,
            koin_balance: -50,
        };
        let account = ga.to_account();
        assert_eq!(account.id, 42);
        assert_eq!(account.oikos_balance, OikosAmount(1000));
        assert_eq!(account.koin_balance.0, -50);
    }

    #[test]
    fn load_from_toml_string() {
        let toml_str = r#"
data_dir = "/tmp/test"
rpc_port = 9999
block_time_ms = 500
log_level = "debug"

[genesis]
treasury_balance = 1000000
chain_id = 42

[[genesis.initial_validators]]
operator_did = "did:example:test"
stake = 100000

[[genesis.initial_accounts]]
id = 1
oikos_balance = 500
koin_balance = 100
"#;
        let config: NodeConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.rpc_port, 9999);
        assert_eq!(config.block_time_ms, 500);
        assert_eq!(config.genesis.chain_id, 42);
        assert_eq!(config.genesis.initial_validators.len(), 1);
        assert_eq!(config.genesis.initial_accounts[0].oikos_balance, 500);
    }

    #[test]
    fn database_path_derived() {
        let config = NodeConfig {
            data_dir: "/var/lib/node".to_string(),
            ..Default::default()
        };
        assert_eq!(config.database_path(), "/var/lib/node/chain.db");
    }
}
