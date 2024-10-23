use chrono::{DateTime, Duration, Utc};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

pub struct LurkServerStats {
    is_started: AtomicBool,
    started_ts_millis: AtomicI64,
}

impl LurkServerStats {
    pub fn new() -> LurkServerStats {
        LurkServerStats {
            started_ts_millis: AtomicI64::new(0),
            is_started: AtomicBool::new(false),
        }
    }

    /// Called when node is started to accept connections.
    pub fn on_server_started(&self) {
        assert!(!self.is_started.load(Ordering::Relaxed), "server shoudn't be started yet");
        let current_time = Utc::now();

        self.is_started.store(true, Ordering::Relaxed);
        self.started_ts_millis.store(current_time.timestamp_millis(), Ordering::Relaxed);
    }

    pub fn on_server_finished(&self) {
        /* Not implemented */
    }

    /// Returns true if server is started.
    /// There's no guarantee it hasn't finished yet.
    pub fn is_server_started(&self) -> bool {
        self.is_started.load(Ordering::Relaxed)
    }

    /// Returns time past since server is started.
    pub fn get_uptime(&self) -> Duration {
        assert!(self.is_started.load(Ordering::Relaxed), "server should be already started");
        let current_ts = Utc::now();
        let started_ts = self.get_started_utc_timestamp();

        assert!(current_ts >= started_ts);
        current_ts - started_ts
    }

    /// Returns UTC timestamp describing server start time.
    pub fn get_started_utc_timestamp(&self) -> DateTime<Utc> {
        assert!(self.is_started.load(Ordering::Relaxed), "server should be already started");
        DateTime::from_timestamp_millis(self.started_ts_millis.load(Ordering::Relaxed)).expect("valid datetime")
    }
}

impl Default for LurkServerStats {
    fn default() -> Self {
        Self::new()
    }
}
