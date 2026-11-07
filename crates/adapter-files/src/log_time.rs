// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated UTC game-log timestamps.

use tallyline_shared_kernel::api::model::Timestamp;

pub(crate) fn parse(text: &str) -> Option<Timestamp> {
    let value = text.strip_prefix('<')?.split_once('>')?.0;
    if value.len() != 24
        || value.get(4..5)? != "-"
        || value.get(7..8)? != "-"
        || value.get(10..11)? != "T"
        || value.get(13..14)? != ":"
        || value.get(16..17)? != ":"
        || value.get(19..20)? != "."
        || value.get(23..24)? != "Z"
    {
        return None;
    }
    let number = |range: std::ops::Range<usize>| {
        let digits = value.get(range)?;
        if !digits.bytes().all(|digit| digit.is_ascii_digit()) {
            return None;
        }
        digits.parse::<u64>().ok()
    };
    let year = number(0..4)?;
    let month = number(5..7)?;
    let day = number(8..10)?;
    let hour = number(11..13)?;
    let minute = number(14..16)?;
    let second = number(17..19)?;
    let millisecond = number(20..23)?;
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    let days_in_month = |m| match m {
        2 if leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day == 0 || day > days_in_month(month) {
        return None;
    }
    let preceding_days: u64 = (1970..year).map(|y| if leap(y) { 366 } else { 365 }).sum();
    let month_days: u64 = (1..month).map(days_in_month).sum();
    Some(Timestamp::from_millis(
        ((((preceding_days + month_days + day - 1) * 24 + hour) * 3600 + minute * 60 + second)
            * 1000)
            + millisecond,
    ))
}
