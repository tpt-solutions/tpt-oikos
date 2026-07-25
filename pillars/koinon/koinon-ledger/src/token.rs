#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenId {
    Oikos,
    Koin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OikosAmount(pub u128);

impl OikosAmount {
    pub const ZERO: Self = Self(0);
    pub const MAX_SUPPLY: u128 = 1_000_000_000 * 10_u128.pow(18);

    pub fn new(units: u128) -> Self {
        Self(units)
    }

    pub fn from_tokens(tokens: u64) -> Self {
        Self(tokens as u128 * 10_u128.pow(18))
    }

    pub fn to_tokens_f64(&self) -> f64 {
        self.0 as f64 / 10_f64.powi(18)
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}

impl std::fmt::Display for OikosAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let int = self.0 / 10_u128.pow(18);
        let frac = self.0 % 10_u128.pow(18);
        write!(f, "{int}.{frac:018}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KoinAmount(pub i128);

impl KoinAmount {
    pub const ZERO: Self = Self(0);

    pub fn new(amount: i128) -> Self {
        Self(amount)
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }
}

impl std::fmt::Display for KoinAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct OikosToken;
pub struct KoinToken;
