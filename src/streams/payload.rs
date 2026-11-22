use std::alloc::Allocator;
use std::fmt;
use std::ops::Deref;
use std::panic::RefUnwindSafe;
use std::sync::Arc;

/// An allocator for received RTP payloads.
///
/// See [`RtcConfig::set_rtp_payload_allocator`][crate::RtcConfig::set_rtp_payload_allocator].
pub type PayloadAllocator = &'static (dyn Allocator + Send + Sync + RefUnwindSafe);

/// The payload of an RTP packet, without the RTP header.
///
/// A payload is either shared, as an `Arc<[u8]>`, or held in memory from a
/// [`PayloadAllocator`]. Cloning a shared payload only counts a reference.
/// Cloning an allocated payload copies its bytes into a new allocation from
/// the same allocator.
#[derive(Clone)]
pub enum Payload {
    /// A payload in shared memory from the global allocator.
    Shared(Arc<[u8]>),
    /// A payload in memory from a [`PayloadAllocator`].
    Allocated(Box<[u8], PayloadAllocator>),
}

impl Payload {
    pub(crate) fn new_in(data: &[u8], allocator: PayloadAllocator) -> Self {
        let mut v = Vec::with_capacity_in(data.len(), allocator);
        v.extend_from_slice(data);
        Payload::Allocated(v.into_boxed_slice())
    }

    /// Turn the payload into an `Arc<[u8]>`.
    ///
    /// A shared payload is returned as it is. An allocated payload is copied.
    pub fn into_shared(self) -> Arc<[u8]> {
        match self {
            Payload::Shared(a) => a,
            Payload::Allocated(b) => Arc::from(&b[..]),
        }
    }
}

impl Default for Payload {
    fn default() -> Self {
        Payload::Shared(Arc::default())
    }
}

/// A [`PayloadAllocator`] in the config, which has no `Debug` of its own.
#[derive(Clone, Copy)]
pub(crate) struct AllocatorSlot(pub PayloadAllocator);

impl fmt::Debug for AllocatorSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PayloadAllocator")
    }
}

impl Deref for Payload {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Payload::Shared(a) => a,
            Payload::Allocated(b) => b,
        }
    }
}

impl AsRef<[u8]> for Payload {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self[..] == other[..]
    }
}

impl Eq for Payload {}

impl fmt::Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Payload").field(&self.len()).finish()
    }
}

impl From<Arc<[u8]>> for Payload {
    fn from(v: Arc<[u8]>) -> Self {
        Payload::Shared(v)
    }
}

impl From<Vec<u8>> for Payload {
    fn from(v: Vec<u8>) -> Self {
        Payload::Shared(v.into())
    }
}

impl From<Box<[u8]>> for Payload {
    fn from(v: Box<[u8]>) -> Self {
        Payload::Shared(v.into())
    }
}

impl From<&[u8]> for Payload {
    fn from(v: &[u8]) -> Self {
        Payload::Shared(v.into())
    }
}

impl From<&Vec<u8>> for Payload {
    fn from(v: &Vec<u8>) -> Self {
        Payload::Shared(v[..].into())
    }
}

impl<const N: usize> From<[u8; N]> for Payload {
    fn from(v: [u8; N]) -> Self {
        Payload::Shared(Arc::new(v))
    }
}

impl<const N: usize> From<&[u8; N]> for Payload {
    fn from(v: &[u8; N]) -> Self {
        Payload::Shared(v[..].into())
    }
}

impl From<Payload> for Arc<[u8]> {
    fn from(v: Payload) -> Self {
        v.into_shared()
    }
}
