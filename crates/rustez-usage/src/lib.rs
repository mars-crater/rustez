//! `rustez-usage`: subscription-limit % only (spendings deferred to TODO).

use serde::{Deserialize, Serialize};

/// One quota/limit, e.g. `requests 5k/10k (50%)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzSubLimit {
    pub id: String,
    pub label: String,
    pub used: u64,
    pub total: u64,
}

impl EzSubLimit {
    /// Percentage 0..=100.
    pub fn pct(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.used as f64 / self.total as f64 * 100.0).min(100.0)
        }
    }
}

/// Subscription usage snapshot (multiple limits supported).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EzSubUsage {
    pub provider: String,
    pub fetched_at: String,
    pub limits: Vec<EzSubLimit>,
    #[serde(default)]
    pub note: String,
}
