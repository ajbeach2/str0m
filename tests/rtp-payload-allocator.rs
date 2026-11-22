//! With an RTP payload allocator set, each received payload comes from that
//! allocator and holds the bytes of its own packet.

use std::alloc::{AllocError, Allocator, Global, Layout};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use str0m::media::MediaKind;
use str0m::rtp::{Payload, RtpWrite, Ssrc};
use str0m::{Event, Rtc, RtcError};

mod common;
use common::{connect_l_r_with_rtc, init_crypto_default, init_log, progress};

struct Counting {
    live: AtomicU64,
    total: AtomicU64,
}

unsafe impl Allocator for Counting {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.live.fetch_add(1, Ordering::Relaxed);
        self.total.fetch_add(1, Ordering::Relaxed);
        Global.allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.live.fetch_sub(1, Ordering::Relaxed);
        unsafe { Global.deallocate(ptr, layout) }
    }
}

static COUNTING: Counting = Counting {
    live: AtomicU64::new(0),
    total: AtomicU64::new(0),
};

fn payload_of(i: u64) -> Vec<u8> {
    vec![i as u8; 40 + (i % 7) as usize]
}

#[test]
fn payloads_come_from_the_allocator() -> Result<(), RtcError> {
    init_log();
    init_crypto_default();

    let now = Instant::now();
    let rtc1 = Rtc::builder().set_rtp_mode(true).build(now);
    let rtc2 = Rtc::builder()
        .set_rtp_mode(true)
        .set_rtp_payload_allocator(Some(&COUNTING))
        .build(now);
    let (mut l, mut r) = connect_l_r_with_rtc(rtc1, rtc2);

    let mid = "audio".into();
    let ssrc: Ssrc = 1337.into();
    l.direct_api().declare_media(mid, MediaKind::Audio);
    l.direct_api().declare_stream_tx(ssrc, None, mid, None);
    r.direct_api().declare_media(mid, MediaKind::Audio);
    r.direct_api().expect_stream_rx(ssrc, None, mid, None);
    let max = l.last.max(r.last);
    l.last = max;
    r.last = max;
    let pt = l.params_opus().pt();

    let mut received = 0;
    for i in 0..60 {
        let wallclock = l.start + l.duration();
        l.direct_api()
            .stream_tx(&ssrc)
            .unwrap()
            .write_rtp(RtpWrite::new(
                pt,
                (1000 + i).into(),
                (i * 960) as u32,
                wallclock,
                payload_of(i),
            ));
        progress(&mut l, &mut r)?;
        for (_, e) in r.events.drain(..) {
            if let Event::RtpPacket(p) = e {
                assert!(matches!(p.payload, Payload::Allocated(_)));
                assert_eq!(&p.payload[..], &payload_of(*p.seq_no - 1000)[..]);
                received += 1;
            }
        }
    }

    assert!(received > 50, "received {received}");
    assert!(COUNTING.total.load(Ordering::Relaxed) >= received);
    assert_eq!(
        COUNTING.live.load(Ordering::Relaxed),
        0,
        "every payload is freed"
    );
    Ok(())
}
