use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::Instant,
};

use serde::Serialize;

/// Per-user execution quotas. In-memory on purpose: an abuse counter does not
/// need to survive a restart for a personal deployment.
pub struct RateLimiter {
    window: Mutex<HashMap<String, Usage>>,
    per_minute: usize,
    per_day: usize,
}

#[derive(Default)]
struct Usage {
    minute: VecDeque<Instant>,
    day: VecDeque<Instant>,
}

#[derive(Debug)]
pub struct QuotaError {
    pub message: String,
    pub retry_after_secs: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct QuotaStatus {
    pub limit_per_minute: usize,
    pub remaining_minute: usize,
    pub limit_per_day: usize,
    pub remaining_day: usize,
}

impl RateLimiter {
    pub fn new(per_minute: usize, per_day: usize) -> Self {
        Self {
            window: Mutex::new(HashMap::new()),
            per_minute,
            per_day,
        }
    }

    pub fn consume(&self, user_id: &str) -> Result<QuotaStatus, QuotaError> {
        let now = Instant::now();
        let mut map = self.window.lock().unwrap_or_else(|e| e.into_inner());
        let usage = map.entry(user_id.to_string()).or_default();

        while usage
            .minute
            .front()
            .is_some_and(|t| now.duration_since(*t).as_secs() >= 60)
        {
            usage.minute.pop_front();
        }
        while usage
            .day
            .front()
            .is_some_and(|t| now.duration_since(*t).as_secs() >= 86_400)
        {
            usage.day.pop_front();
        }

        if usage.minute.len() >= self.per_minute {
            let retry = usage
                .minute
                .front()
                .map(|t| 60u64.saturating_sub(now.duration_since(*t).as_secs()))
                .unwrap_or(60);
            return Err(QuotaError {
                message: format!(
                    "quota atteint : {} exécutions par minute. Réessayez dans {} s.",
                    self.per_minute, retry
                ),
                retry_after_secs: retry,
            });
        }
        if usage.day.len() >= self.per_day {
            return Err(QuotaError {
                message: format!(
                    "quota atteint : {} exécutions par jour. Réessayez demain.",
                    self.per_day
                ),
                retry_after_secs: 3600,
            });
        }

        usage.minute.push_back(now);
        usage.day.push_back(now);

        Ok(QuotaStatus {
            limit_per_minute: self.per_minute,
            remaining_minute: self.per_minute.saturating_sub(usage.minute.len()),
            limit_per_day: self.per_day,
            remaining_day: self.per_day.saturating_sub(usage.day.len()),
        })
    }

    pub fn status(&self, user_id: &str) -> QuotaStatus {
        let now = Instant::now();
        let mut map = self.window.lock().unwrap_or_else(|e| e.into_inner());
        let usage = map.entry(user_id.to_string()).or_default();
        while usage
            .minute
            .front()
            .is_some_and(|t| now.duration_since(*t).as_secs() >= 60)
        {
            usage.minute.pop_front();
        }
        while usage
            .day
            .front()
            .is_some_and(|t| now.duration_since(*t).as_secs() >= 86_400)
        {
            usage.day.pop_front();
        }
        QuotaStatus {
            limit_per_minute: self.per_minute,
            remaining_minute: self.per_minute.saturating_sub(usage.minute.len()),
            limit_per_day: self.per_day,
            remaining_day: self.per_day.saturating_sub(usage.day.len()),
        }
    }
}
