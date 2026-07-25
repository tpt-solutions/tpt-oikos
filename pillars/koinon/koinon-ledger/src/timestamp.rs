use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub const ZERO: Self = Self(0);

    pub fn new(millis: u64) -> Self {
        Self(millis)
    }

    pub fn is_expired(&self, now: Timestamp) -> bool {
        now.0 > self.0
    }

    pub fn is_valid(&self, now: Timestamp) -> bool {
        self.0 == 0 || !self.is_expired(now)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Timestamp({})", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContractAddress(pub String);

impl ContractAddress {
    pub fn new(address: impl Into<String>) -> Self {
        Self(address.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContractAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ContractAddress {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}
