use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;

use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};

use super::api::LogLevel;
use super::platform;

const HIGH_PRIORITY_QUEUE_CAPACITY: usize = 256;
const LOW_PRIORITY_QUEUE_CAPACITY: usize = 1024;

#[derive(Debug)]
pub(super) struct LogRecord {
    pub(super) level: LogLevel,
    pub(super) tag: Arc<str>,
    pub(super) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QueueKind {
    High,
    Low,
}

#[derive(Debug)]
struct QueueSlots {
    len: AtomicUsize,
    capacity: usize,
}

impl QueueSlots {
    fn new(capacity: usize) -> Self {
        Self {
            len: AtomicUsize::new(0),
            capacity,
        }
    }

    fn try_reserve(&self) -> bool {
        let mut current = self.len.load(Ordering::Relaxed);
        loop {
            if current >= self.capacity {
                return false;
            }

            match self.len.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(next) => current = next,
            }
        }
    }

    fn release(&self) {
        self.len.fetch_sub(1, Ordering::Release);
    }
}

#[derive(Debug)]
pub(super) struct LogDispatcher {
    high_tx: Sender<LogRecord>,
    low_tx: Sender<LogRecord>,
    high_rx: Receiver<LogRecord>,
    low_rx: Receiver<LogRecord>,
    high_slots: Arc<QueueSlots>,
    low_slots: Arc<QueueSlots>,
}

impl LogDispatcher {
    fn new() -> Self {
        let (high_tx, high_rx) = unbounded();
        let (low_tx, low_rx) = unbounded();
        let high_slots = Arc::new(QueueSlots::new(HIGH_PRIORITY_QUEUE_CAPACITY));
        let low_slots = Arc::new(QueueSlots::new(LOW_PRIORITY_QUEUE_CAPACITY));

        let dispatcher = Self {
            high_tx,
            low_tx,
            high_rx,
            low_rx,
            high_slots,
            low_slots,
        };
        dispatcher.spawn_worker();
        dispatcher
    }

    fn spawn_worker(&self) {
        let high_rx = self.high_rx.clone();
        let low_rx = self.low_rx.clone();
        let high_slots = self.high_slots.clone();
        let low_slots = self.low_slots.clone();

        thread::Builder::new()
            .name("tgui-log".to_string())
            .spawn(move || worker_loop(high_rx, low_rx, high_slots, low_slots))
            .expect("failed to spawn tgui log worker");
    }

    pub(super) fn reserve(&self, level: LogLevel) -> Option<QueueKind> {
        match level {
            LogLevel::Warn | LogLevel::Error => {
                if self.high_slots.try_reserve() {
                    Some(QueueKind::High)
                } else if self.low_slots.try_reserve() {
                    Some(QueueKind::Low)
                } else {
                    None
                }
            }
            LogLevel::Trace | LogLevel::Debug | LogLevel::Info => {
                if self.low_slots.try_reserve() {
                    Some(QueueKind::Low)
                } else {
                    None
                }
            }
        }
    }

    pub(super) fn dispatch(
        &self,
        reservation: QueueKind,
        record: LogRecord,
    ) -> Result<(), LogRecord> {
        let send_result = match reservation {
            QueueKind::High => self.high_tx.send(record),
            QueueKind::Low => self.low_tx.send(record),
        };

        send_result.map_err(|error| error.0)
    }

    pub(super) fn release(&self, reservation: QueueKind) {
        match reservation {
            QueueKind::High => self.high_slots.release(),
            QueueKind::Low => self.low_slots.release(),
        }
    }

    #[cfg(test)]
    pub(super) fn try_drain_one(&self) -> Option<LogRecord> {
        if let Ok(record) = self.high_rx.try_recv() {
            self.high_slots.release();
            return Some(record);
        }

        if let Ok(record) = self.low_rx.try_recv() {
            self.low_slots.release();
            return Some(record);
        }

        None
    }
}

fn worker_loop(
    high_rx: Receiver<LogRecord>,
    low_rx: Receiver<LogRecord>,
    high_slots: Arc<QueueSlots>,
    low_slots: Arc<QueueSlots>,
) {
    loop {
        select_biased! {
            recv(high_rx) -> record => match record {
                Ok(record) => {
                    emit_record(record);
                    high_slots.release();
                }
                Err(_) => return,
            },
            recv(low_rx) -> record => match record {
                Ok(record) => {
                    emit_record(record);
                    low_slots.release();
                }
                Err(_) => return,
            },
        }
    }
}

fn emit_record(record: LogRecord) {
    platform::write(record.level, &record.tag, &record.message);
}

pub(super) fn logger() -> &'static LogDispatcher {
    static LOGGER: OnceLock<LogDispatcher> = OnceLock::new();
    LOGGER.get_or_init(LogDispatcher::new)
}

#[cfg(test)]
impl LogDispatcher {
    pub(super) fn new_for_test(high_capacity: usize, low_capacity: usize) -> Self {
        let (high_tx, high_rx) = unbounded();
        let (low_tx, low_rx) = unbounded();
        Self {
            high_tx,
            low_tx,
            high_rx,
            low_rx,
            high_slots: Arc::new(QueueSlots::new(high_capacity)),
            low_slots: Arc::new(QueueSlots::new(low_capacity)),
        }
    }
}
