//! `Rtc::poll_output_into` puts each received RTP payload in the caller's
//! buffer, and `Rtc::poll_output` still puts it in the packet.

use std::time::{Duration, Instant};

use str0m::media::MediaKind;
use str0m::net::Receive;
use str0m::rtp::{RtpWrite, Ssrc};
use str0m::{Event, Input, Output, Rtc, RtcError};

mod common;
use common::{connect_l_r_with_rtc, init_crypto_default, init_log};

fn payload_of(i: u64) -> Vec<u8> {
    vec![i as u8; 40 + (i % 7) as usize]
}

#[test]
fn payload_lands_in_caller_buffer() -> Result<(), RtcError> {
    init_log();
    init_crypto_default();

    let now = Instant::now();
    let rtc1 = Rtc::builder().set_rtp_mode(true).build(now);
    let rtc2 = Rtc::builder().set_rtp_mode(true).build(now);
    let (mut l, mut r) = connect_l_r_with_rtc(rtc1, rtc2);

    let mid = "audio".into();
    let ssrc: Ssrc = 1337.into();
    l.direct_api().declare_media(mid, MediaKind::Audio);
    l.direct_api().declare_stream_tx(ssrc, None, mid, None);
    r.direct_api().declare_media(mid, MediaKind::Audio);
    r.direct_api().expect_stream_rx(ssrc, None, mid, None);
    let pt = l.params_opus().pt();
    let start = l.last.max(r.last);

    let mut buf = Vec::new();
    let mut into = 0;
    let mut plain = 0;

    for i in 0..40 {
        let t = start + Duration::from_millis(20 * i);
        l.direct_api()
            .stream_tx(&ssrc)
            .unwrap()
            .write_rtp(RtpWrite::new(
                pt,
                (1000 + i).into(),
                (i * 960) as u32,
                t,
                payload_of(i),
            ));
        l.rtc.handle_input(Input::Timeout(t))?;
        let mut sent = Vec::new();
        loop {
            match l.rtc.poll_output()? {
                Output::Timeout(_) => break,
                Output::Transmit(v) => sent.push(v),
                Output::Event(_) => {}
            }
        }

        r.rtc.handle_input(Input::Timeout(t))?;
        for v in sent {
            r.rtc.handle_input(Input::Receive(
                t,
                Receive {
                    proto: v.proto,
                    source: v.source,
                    destination: v.destination,
                    contents: (&v.contents[..]).try_into()?,
                },
            ))?;
        }

        // Odd packets go to the caller's buffer and even packets keep their payload.
        let use_buf = i % 2 == 1;
        loop {
            let o = if use_buf {
                r.rtc.poll_output_into(&mut buf)?
            } else {
                r.rtc.poll_output()?
            };
            match o {
                Output::Timeout(_) => break,
                Output::Event(Event::RtpPacket(p)) => {
                    let want = payload_of(*p.seq_no - 1000);
                    if use_buf {
                        assert!(p.payload.is_empty(), "packet {i} payload is empty");
                        assert_eq!(&buf[..], &want[..], "packet {i} in the buffer");
                        into += 1;
                    } else {
                        assert_eq!(&p.payload[..], &want[..], "packet {i} in the packet");
                        plain += 1;
                    }
                }
                _ => {}
            }
        }
    }

    assert!(into > 15 && plain > 15, "into {into} plain {plain}");
    Ok(())
}
