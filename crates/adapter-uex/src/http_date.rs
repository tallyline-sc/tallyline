// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded UTC HTTP-date translation into the unsigned Unix timestamp range.

use tallyline_shared_kernel::api::model::Timestamp;

const MONTH_LENGTHS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const SECONDS_PER_DAY: u64 = 86400;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Date {
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

pub(crate) fn parse(value: &str, now: Timestamp) -> Option<Timestamp> {
    if value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    {
        return None;
    }
    let tokens: Vec<_> = value.split_whitespace().collect();
    let (weekday, date) = match tokens.as_slice() {
        [name, day, month, year, time, "GMT"] if year.len() == 4 && day.len() == 2 => (
            weekday(name.strip_suffix(',')?)?,
            parts(year, month, day, time)?,
        ),
        [name, date, time, "GMT"] => {
            let weekday = weekday(name.strip_suffix(',')?)?;
            let fields: Vec<_> = date.split('-').collect();
            let [day, month, year] = fields.as_slice() else {
                return None;
            };
            if day.len() != 2 || year.len() != 2 {
                return None;
            }
            let current = at(now)?;
            let year = current.year / 100 * 100 + integer(year)?;
            let mut result = parts(&year.to_string(), month, day, time)?;
            let threshold = Date {
                year: current.year.checked_add(50)?,
                ..current
            };
            if result > threshold {
                result.year = result.year.checked_sub(100)?;
            }
            (weekday, result)
        }
        [name, month, day, time, year] if year.len() == 4 && day.len() <= 2 => {
            (weekday(name)?, parts(year, month, day, time)?)
        }
        _ => return None,
    };
    let result = date.timestamp()?;
    if (result.seconds() / SECONDS_PER_DAY + 4) % 7 != weekday {
        return None;
    }
    Some(result)
}
fn parts(year: &str, month: &str, day: &str, time: &str) -> Option<Date> {
    let fields: Vec<_> = time.split(':').collect();
    let [hour, minute, second] = fields.as_slice() else {
        return None;
    };
    if hour.len() != 2 || minute.len() != 2 || second.len() != 2 {
        return None;
    }
    Some(Date {
        year: integer(year)?,
        month: month_number(month)?,
        day: integer(day)?,
        hour: integer(hour)?,
        minute: integer(minute)?,
        second: integer(second)?,
    })
}
fn integer(value: &str) -> Option<u32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        None
    } else {
        value.parse().ok()
    }
}
fn weekday(value: &str) -> Option<u64> {
    match value {
        "Sun" | "Sunday" => Some(0),
        "Mon" | "Monday" => Some(1),
        "Tue" | "Tuesday" => Some(2),
        "Wed" | "Wednesday" => Some(3),
        "Thu" | "Thursday" => Some(4),
        "Fri" | "Friday" => Some(5),
        "Sat" | "Saturday" => Some(6),
        _ => None,
    }
}
fn month_number(value: &str) -> Option<u32> {
    match value {
        "Jan" => Some(1),
        "Feb" => Some(2),
        "Mar" => Some(3),
        "Apr" => Some(4),
        "May" => Some(5),
        "Jun" => Some(6),
        "Jul" => Some(7),
        "Aug" => Some(8),
        "Sep" => Some(9),
        "Oct" => Some(10),
        "Nov" => Some(11),
        "Dec" => Some(12),
        _ => None,
    }
}
fn leap(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}
fn month_length(year: u32, month: u32) -> Option<u32> {
    let length = *MONTH_LENGTHS.get(usize::try_from(month.checked_sub(1)?).ok()?)?;
    Some(length + u32::from(month == 2 && leap(year)))
}
fn leaps_through(year: u32) -> u64 {
    u64::from(year / 4 - year / 100 + year / 400)
}
fn year_start(year: u32) -> Option<u64> {
    Some(
        u64::from(year.checked_sub(1970)?) * 365 + leaps_through(year.checked_sub(1)?)
            - leaps_through(1969),
    )
}
impl Date {
    fn timestamp(self) -> Option<Timestamp> {
        if self.year > 9999
            || self.day == 0
            || self.day > month_length(self.year, self.month)?
            || self.hour > 23
            || self.minute > 59
            || self.second > 60
        {
            return None;
        }
        let mut days = year_start(self.year)?;
        for month in 1..self.month {
            days += u64::from(month_length(self.year, month)?);
        }
        days += u64::from(self.day - 1);
        Some(Timestamp::from_seconds(
            days * SECONDS_PER_DAY
                + u64::from(self.hour) * 3600
                + u64::from(self.minute) * 60
                + u64::from(self.second.min(59)),
        ))
    }
}
fn at(timestamp: Timestamp) -> Option<Date> {
    let days = timestamp.seconds() / SECONDS_PER_DAY;
    if days >= year_start(10000)? {
        return None;
    }
    let mut low = 1970;
    let mut high = 10000;
    while low + 1 < high {
        let middle = (low + high) / 2;
        if year_start(middle)? <= days {
            low = middle;
        } else {
            high = middle;
        }
    }
    let year = low;
    let mut remaining = days - year_start(year)?;
    let mut month = 1;
    while remaining >= u64::from(month_length(year, month)?) {
        remaining -= u64::from(month_length(year, month)?);
        month += 1;
    }
    let clock = timestamp.seconds() % SECONDS_PER_DAY;
    Some(Date {
        year,
        month,
        day: u32::try_from(remaining).ok()? + 1,
        hour: u32::try_from(clock / 3600).ok()?,
        minute: u32::try_from(clock % 3600 / 60).ok()?,
        second: u32::try_from(clock % 60).ok()?,
    })
}
