#[derive(Debug, Clone)]
pub struct GasConfig {
    pub base_cost: u64,
    pub per_step: u64,
    pub per_storage_byte: u64,
}

impl Default for GasConfig {
    fn default() -> Self {
        Self {
            base_cost: 1000,
            per_step: 10,
            per_storage_byte: 100,
        }
    }
}

impl GasConfig {
    pub fn new(base_cost: u64, per_step: u64, per_storage_byte: u64) -> Self {
        Self {
            base_cost,
            per_step,
            per_storage_byte,
        }
    }
}
