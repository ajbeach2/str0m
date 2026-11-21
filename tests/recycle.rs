//! Handing a sent datagram back with `Rtc::recycle` changes no output and no
//! timeout. The shared test harness recycles every datagram it sends, so the
//! rest of the suite covers the reuse itself.

use std::time::Instant;

use str0m::net::DatagramSend;
use str0m::{Input, Output, Rtc, RtcError};

mod common;
use common::init_crypto_default;

#[test]
fn recycling_changes_no_output_and_no_timeout() -> Result<(), RtcError> {
    init_crypto_default();
    let now = Instant::now();
    let mut rtc = Rtc::new(now);
    rtc.handle_input(Input::Timeout(now))?;
    let deadline = loop {
        if let Output::Timeout(t) = rtc.poll_output()? {
            break t;
        }
    };

    rtc.recycle(DatagramSend::from(vec![0_u8; 1200]));
    rtc.recycle(DatagramSend::from(Vec::new()));

    match rtc.poll_output()? {
        Output::Timeout(t) => assert_eq!(t, deadline, "the deadline stays"),
        output => panic!("recycling must make no output ready: {output:?}"),
    }
    Ok(())
}
