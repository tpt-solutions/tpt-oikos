#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MandateScope {
    TransferOikos,
    TransferKoin,
    MintKoin,
    BurnKoin,
    EscrowManage,
    StreamManage,
    RfpPublish,
    RfpRespond,
    DelegateTo(String),
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct ScopeRule {
    pub scope: MandateScope,
    pub max_per_tx: u64,
    pub max_daily: u64,
}

impl MandateScope {
    pub fn allows_transfer_oikos(&self) -> bool {
        matches!(self, Self::TransferOikos)
    }

    pub fn allows_transfer_koin(&self) -> bool {
        matches!(self, Self::TransferKoin)
    }

    pub fn allows_escrow(&self) -> bool {
        matches!(self, Self::EscrowManage)
    }

    pub fn allows_stream(&self) -> bool {
        matches!(self, Self::StreamManage)
    }
}
