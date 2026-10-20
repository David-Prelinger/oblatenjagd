//! Running independent jobs on scoped threads with ordered results.
//!
//! Used by the verify steps whose items do not depend on each other (the
//! replays, the golden shots, the solver's levels). Each item is processed
//! exactly as it would be on one thread; only the wall-clock time changes.
//! The results come back in item order, so every report is identical for
//! any thread count.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Threads for `map_ordered` in the verify steps: the machine's parallelism.
pub fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// `items.iter().map(f).collect()`, computed on up to `threads` scoped
/// threads. Work is handed out one item at a time (the next free thread
/// takes the next item), so a few slow items do not hold up a whole chunk.
/// The result is in item order whatever the thread count.
pub fn map_ordered<T: Sync, R: Send>(
    items: &[T],
    threads: usize,
    f: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    let threads = threads.min(items.len());
    if threads <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        out.push((i, f(item)));
                    }
                    out
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("worker thread panicked"))
            .collect()
    });
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_are_in_item_order_for_any_thread_count() {
        let items: Vec<u64> = (0..97).collect();
        // Uneven work so threads finish out of order.
        let f = |&n: &u64| -> u64 {
            let mut x = n;
            for _ in 0..(n % 7) * 10_000 {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
            }
            x
        };
        let serial: Vec<u64> = items.iter().map(f).collect();
        for threads in [0, 1, 2, 3, 8, 200] {
            assert_eq!(map_ordered(&items, threads, f), serial, "{threads} threads");
        }
    }

    #[test]
    fn every_item_is_processed_once() {
        let calls = AtomicUsize::new(0);
        let items = vec![(); 50];
        let out = map_ordered(&items, 4, |_| calls.fetch_add(1, Ordering::Relaxed));
        assert_eq!(out.len(), 50);
        assert_eq!(calls.load(Ordering::Relaxed), 50);
        let mut seen = out.clone();
        seen.sort();
        assert_eq!(seen, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn empty_input() {
        let out: Vec<u8> = map_ordered(&[] as &[u8], 4, |&b| b);
        assert!(out.is_empty());
    }
}
