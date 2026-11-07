// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Published shared time and identity contracts.

use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp};

#[test]
fn timestamp_preserves_milliseconds_and_the_full_seconds_range() {
    let capture = Timestamp::from_millis(1_701_234_567_891);
    assert_eq!(capture.seconds(), 1_701_234_567);
    assert_eq!(capture.millis(), 1_701_234_567_891);
    assert!(capture > Timestamp::from_millis(1_701_234_567_890));
    let maximum = Timestamp::from_seconds(u64::MAX);
    assert_eq!(maximum.seconds(), u64::MAX);
    assert_eq!(maximum.millis(), 18_446_744_073_709_551_615_000);
    assert_eq!(Timestamp::from_seconds(0).millis(), 0);
    assert_eq!(CommodityId(42).0, 42);
    assert_eq!(TerminalId(89).0, 89);
}
