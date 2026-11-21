//! With the payload pool on, each received payload holds the bytes of its
//! own packet, and a payload the caller still holds is never overwritten.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use str0m::media::MediaKind;
use str0m::rtp::{RtpWrite, Ssrc};
use str0m::{Event, Rtc, RtcError};

mod common;
use common::{connect_l_r_with_rtc, init_crypto_default, init_log, progress};

const PACKETS: u64 = 120;

fn payload_of(i: u64) -> Vec<u8> {
    // Seven lengths, and bytes that differ for each packet.
    vec![i as u8; 40 + (i % 7) as usize]
}

#[test]
fn pooled_payloads_hold_their_own_bytes() -> Result<(), RtcError> {
    init_log();
    init_crypto_default();

    let now = Instant::now();
    let rtc1 = Rtc::builder().set_rtp_mode(true).build(now);
    let rtc2 = Rtc::builder()
        .set_rtp_mode(true)
        .set_rtp_payload_pool(16)
        .build(now);

    let (mut l, mut r) = connect_l_r_with_rtc(rtc1, rtc2);

    let mid = "audio".into();
    let ssrc_tx: Ssrc = 1337.into();
    l.direct_api().declare_media(mid, MediaKind::Audio);
    l.direct_api().declare_stream_tx(ssrc_tx, None, mid, None);
    r.direct_api().declare_media(mid, MediaKind::Audio);
    r.direct_api().expect_stream_rx(ssrc_tx, None, mid, None);

    let max = l.last.max(r.last);
    l.last = max;
    r.last = max;

    let pt = l.params_opus().pt();

    let mut received = 0;
    // The caller keeps the last few payloads while the pool reuses others.
    let mut held: VecDeque<(u64, Arc<[u8]>)> = VecDeque::new();

    for i in 0..PACKETS {
        let wallclock = l.start + l.duration();
        l.direct_api()
            .stream_tx(&ssrc_tx)
            .unwrap()
            .write_rtp(RtpWrite::new(
                pt,
                (1000 + i).into(),
                (i * 960) as u32,
                wallclock,
                payload_of(i),
            ));
        progress(&mut l, &mut r)?;

        // Check each payload. Keep the newest three and let go of the rest,
        // as a forwarder would once a packet is sent.
        for (_, e) in r.events.drain(..) {
            if let Event::RtpPacket(p) = e {
                let i = *p.seq_no - 1000;
                assert_eq!(&p.payload[..], &payload_of(i)[..], "packet {i}");
                held.push_back((i, p.payload));
                if held.len() > 3 {
                    held.pop_front();
                }
                received += 1;
            }
        }
        for (i, payload) in &held {
            assert_eq!(&payload[..], &payload_of(*i)[..], "held packet {i}");
        }
    }

    assert!(received > PACKETS - 10, "received {received}");

    Ok(())
}
