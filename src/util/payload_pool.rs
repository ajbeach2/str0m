use std::sync::Arc;

/// Reuses the allocations of received RTP payloads.
///
/// The pool keeps a clone of each payload it hands out. Once every other
/// clone is dropped, the pool holds the only reference, and
/// [`Arc::get_mut`] lets it overwrite the bytes of a payload of the same
/// length. A payload that is still held elsewhere is never touched.
///
/// A pool of size 0 keeps nothing and allocates every payload, as before.
#[derive(Debug)]
pub(crate) struct PayloadPool {
    max: usize,
    held: Vec<Arc<[u8]>>,
}

impl PayloadPool {
    pub fn new(max: usize) -> Self {
        PayloadPool {
            max,
            held: Vec::with_capacity(max),
        }
    }

    /// A payload holding a copy of `data`.
    pub fn payload(&mut self, data: &[u8]) -> Arc<[u8]> {
        if self.max == 0 {
            return Arc::from(data);
        }

        // A free payload of the same length takes the bytes in place.
        for slot in self.held.iter_mut() {
            if slot.len() != data.len() {
                continue;
            }
            if let Some(buf) = Arc::get_mut(slot) {
                buf.copy_from_slice(data);
                return slot.clone();
            }
        }

        let fresh: Arc<[u8]> = Arc::from(data);

        if self.held.len() < self.max {
            self.held.push(fresh.clone());
        } else if let Some(slot) = self.held.iter_mut().find(|s| Arc::strong_count(s) == 1) {
            // Full: a free payload of another length makes room, so the pool
            // follows the lengths that arrive. One still in use stays.
            *slot = fresh.clone();
        }

        fresh
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn reuses_a_free_payload_of_the_same_length() {
        let mut pool = PayloadPool::new(4);
        let a = pool.payload(&[1, 2, 3]);
        let ptr = Arc::as_ptr(&a);
        drop(a);
        let b = pool.payload(&[4, 5, 6]);
        assert_eq!(Arc::as_ptr(&b), ptr);
        assert_eq!(&b[..], &[4, 5, 6]);
    }

    #[test]
    fn never_writes_a_payload_still_held() {
        let mut pool = PayloadPool::new(4);
        let a = pool.payload(&[1, 2, 3]);
        let b = pool.payload(&[4, 5, 6]);
        assert_ne!(Arc::as_ptr(&a), Arc::as_ptr(&b));
        assert_eq!(&a[..], &[1, 2, 3]);
        assert_eq!(&b[..], &[4, 5, 6]);
    }

    #[test]
    fn a_weak_reference_also_blocks_reuse() {
        let mut pool = PayloadPool::new(4);
        let a = pool.payload(&[1, 2, 3]);
        let weak = Arc::downgrade(&a);
        drop(a);
        let b = pool.payload(&[4, 5, 6]);
        assert_eq!(&weak.upgrade().unwrap()[..], &[1, 2, 3]);
        assert_eq!(&b[..], &[4, 5, 6]);
    }

    #[test]
    fn a_full_pool_replaces_a_free_payload_of_another_length() {
        let mut pool = PayloadPool::new(1);
        drop(pool.payload(&[1, 2, 3]));
        let b = pool.payload(&[1, 2]);
        let ptr = Arc::as_ptr(&b);
        drop(b);
        assert_eq!(Arc::as_ptr(&pool.payload(&[7, 8])), ptr);
    }

    #[test]
    fn a_pool_of_zero_keeps_nothing() {
        let mut pool = PayloadPool::new(0);
        let a = pool.payload(&[1, 2, 3]);
        assert_eq!(Arc::strong_count(&a), 1);
        assert!(pool.held.is_empty());
    }
}
