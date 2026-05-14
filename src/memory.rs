use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT_TOTAL_RAM_BYTES: usize = 16 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct MemoryGovernor {
    budget_bytes: usize,
    used_bytes: Arc<AtomicU64>,
}

impl MemoryGovernor {
    pub fn one_percent_of(total_ram_bytes: usize) -> Self {
        Self::new((total_ram_bytes / 100).max(64 * 1024))
    }

    pub fn one_percent_assumed_host() -> Self {
        Self::one_percent_of(DEFAULT_TOTAL_RAM_BYTES)
    }

    pub fn new(budget_bytes: usize) -> Self {
        Self {
            budget_bytes,
            used_bytes: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    pub fn used_bytes(&self) -> usize {
        self.used_bytes.load(Ordering::Relaxed) as usize
    }

    pub fn usage_ratio(&self) -> f64 {
        self.used_bytes() as f64 / self.budget_bytes as f64
    }

    pub(crate) fn try_reserve(&self, bytes: usize) -> bool {
        let mut current = self.used_bytes.load(Ordering::Relaxed);
        loop {
            let next = current.saturating_add(bytes as u64);
            if next as usize > self.budget_bytes {
                return false;
            }
            match self.used_bytes.compare_exchange_weak(
                current,
                next,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }

    pub(crate) fn release(&self, bytes: usize) {
        self.used_bytes.fetch_sub(bytes as u64, Ordering::SeqCst);
    }
}
