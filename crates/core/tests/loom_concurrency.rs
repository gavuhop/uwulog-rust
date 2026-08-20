#![cfg(test)]

use loom::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use loom::sync::{Arc, RwLock};
use loom::thread;
use std::collections::VecDeque;

/// Model mô phỏng cơ chế Producer-Consumer RingBuffer & Atomic Counter trong SystemEngine
struct LoomRingBufferModel {
    events: Arc<RwLock<VecDeque<u64>>>,
    total_processed: Arc<AtomicU64>,
    capacity: usize,
}

impl LoomRingBufferModel {
    fn new(capacity: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
            total_processed: Arc::new(AtomicU64::new(0)),
            capacity,
        }
    }

    fn push(&self, item: u64) {
        if let Ok(mut evts) = self.events.write() {
            evts.push_back(item);
            if evts.len() > self.capacity {
                evts.pop_front();
            }
        }
        self.total_processed.fetch_add(1, Ordering::SeqCst);
    }

    fn read_incremental(&self, last_processed: u64) -> Vec<u64> {
        let current_total = self.total_processed.load(Ordering::SeqCst);
        if current_total <= last_processed {
            return Vec::new();
        }

        let new_count = (current_total - last_processed) as usize;

        if let Ok(evts) = self.events.read() {
            let total_in_buffer = evts.len();
            let take_count = new_count.min(total_in_buffer);
            let start_idx = total_in_buffer.saturating_sub(take_count);

            evts.iter().skip(start_idx).cloned().collect()
        } else {
            Vec::new()
        }
    }

    fn clear(&self) {
        if let Ok(mut evts) = self.events.write() {
            evts.clear();
        }
        self.total_processed.store(0, Ordering::SeqCst);
    }
}

#[test]
fn test_loom_producer_consumer_race() {
    loom::model(|| {
        let buffer = Arc::new(LoomRingBufferModel::new(4));

        let b1 = Arc::clone(&buffer);
        let t1 = thread::spawn(move || {
            b1.push(101);
            b1.push(102);
        });

        let b2 = Arc::clone(&buffer);
        let t2 = thread::spawn(move || {
            let logs = b2.read_incremental(0);
            // Verify: logs đọc được luôn hợp lệ và không vượt quá số lượng thực tế
            assert!(logs.len() <= 2);
            for &item in &logs {
                assert!(item == 101 || item == 102);
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Sau khi hoàn thành, buffer phải chứa 2 phần tử và total_processed = 2
        assert_eq!(buffer.total_processed.load(Ordering::SeqCst), 2);
        assert_eq!(buffer.events.read().unwrap().len(), 2);
    });
}

#[test]
fn test_loom_fifo_eviction_under_concurrency() {
    loom::model(|| {
        // Dung lượng tối đa = 2
        let buffer = Arc::new(LoomRingBufferModel::new(2));

        let b1 = Arc::clone(&buffer);
        let t1 = thread::spawn(move || {
            b1.push(1);
            b1.push(2);
            b1.push(3); // Gây ra FIFO eviction (phần tử 1 bị xóa)
        });

        let b2 = Arc::clone(&buffer);
        let t2 = thread::spawn(move || {
            let logs = b2.read_incremental(0);
            // Số phần tử đọc được tối đa là dung lượng buffer (2)
            assert!(logs.len() <= 2);
        });

        t1.join().unwrap();
        t2.join().unwrap();

        let final_logs = buffer.events.read().unwrap();
        assert_eq!(final_logs.len(), 2);
        assert_eq!(final_logs[0], 2);
        assert_eq!(final_logs[1], 3);
    });
}

#[test]
fn test_loom_concurrent_ingest_and_clear_race() {
    loom::model(|| {
        let buffer = Arc::new(LoomRingBufferModel::new(3));

        let b1 = Arc::clone(&buffer);
        let t1 = thread::spawn(move || {
            b1.push(1);
            b1.push(2);
        });

        let b2 = Arc::clone(&buffer);
        let t2 = thread::spawn(move || {
            b2.clear();
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Đảm bảo không deadlock và state sau khi kết thúc là nhất quán
        let total = buffer.total_processed.load(Ordering::SeqCst);
        let len = buffer.events.read().unwrap().len();
        assert!(len <= 2);
        assert!(total <= 2);
    });
}

#[test]
fn test_loom_latch_toggle_synchronization() {
    loom::model(|| {
        let is_auto_scroll = Arc::new(AtomicBool::new(true));

        let flag1 = Arc::clone(&is_auto_scroll);
        let t1 = thread::spawn(move || {
            flag1.store(false, Ordering::SeqCst);
        });

        let flag2 = Arc::clone(&is_auto_scroll);
        let t2 = thread::spawn(move || {
            let current = flag2.load(Ordering::SeqCst);
            flag2.store(!current, Ordering::SeqCst);
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Xác nhận trạng thái boolean kết thúc hợp lệ
        let _final_state = is_auto_scroll.load(Ordering::SeqCst);
    });
}

#[test]
fn test_loom_multi_producers_single_consumer() {
    loom::model(|| {
        let buffer = Arc::new(LoomRingBufferModel::new(4));

        let b1 = Arc::clone(&buffer);
        let p1 = thread::spawn(move || {
            b1.push(10);
        });

        let b2 = Arc::clone(&buffer);
        let p2 = thread::spawn(move || {
            b2.push(20);
        });

        let b3 = Arc::clone(&buffer);
        let c = thread::spawn(move || {
            let logs = b3.read_incremental(0);
            assert!(logs.len() <= 2);
        });

        p1.join().unwrap();
        p2.join().unwrap();
        c.join().unwrap();

        assert_eq!(buffer.total_processed.load(Ordering::SeqCst), 2);
        assert_eq!(buffer.events.read().unwrap().len(), 2);
    });
}

#[test]
fn test_loom_concurrent_readers() {
    loom::model(|| {
        let buffer = Arc::new(LoomRingBufferModel::new(4));
        buffer.push(1);
        buffer.push(2);

        let b1 = Arc::clone(&buffer);
        let r1 = thread::spawn(move || {
            let logs = b1.read_incremental(0);
            assert_eq!(logs.len(), 2);
        });

        let b2 = Arc::clone(&buffer);
        let r2 = thread::spawn(move || {
            let logs = b2.read_incremental(1);
            assert_eq!(logs.len(), 1);
            assert_eq!(logs[0], 2);
        });

        r1.join().unwrap();
        r2.join().unwrap();
    });
}
