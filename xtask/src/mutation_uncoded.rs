// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Uncoded rendered Rust diagnostics bound to the mutated source file.

use serde_json::Value;

pub(super) fn rust_error(text: &str, outcome: &Value) -> bool {
    let Some(file) = outcome
        .pointer("/scenario/Mutant/file")
        .and_then(Value::as_str)
    else {
        return false;
    };
    if !file.ends_with(".rs")
        || file.contains(':')
        || file
            .split(['/', '\\'])
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return false;
    }
    let mut lines = text.lines();
    while let Some(headline) = lines.next() {
        if !headline
            .strip_prefix("error: ")
            .is_some_and(|detail| !detail.trim().is_empty())
        {
            continue;
        }
        let mut block = lines.clone();
        let (Some(span), Some(gutter), Some(source), Some(marker)) =
            (block.next(), block.next(), block.next(), block.next())
        else {
            continue;
        };
        let Some((line, column)) = source_location(span, file) else {
            continue;
        };
        let Some((number, code)) = source.split_once('|') else {
            continue;
        };
        if gutter.trim() == "|"
            && number.trim() == line
            && !code.trim().is_empty()
            && caret_matches(marker, column)
        {
            return true;
        }
    }
    false
}

fn source_location<'a>(span: &'a str, file: &str) -> Option<(&'a str, usize)> {
    let location = span.trim_start().strip_prefix("--> ")?;
    let (location, column) = location.rsplit_once(':')?;
    let (path, line) = location.rsplit_once(':')?;
    if !path.split(['/', '\\']).eq(file.split(['/', '\\'])) {
        return None;
    }
    positive_integer(line)?;
    Some((line, positive_integer(column)?))
}

fn positive_integer(value: &str) -> Option<usize> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok().filter(|number| *number > 0)
}

fn caret_matches(marker: &str, column: usize) -> bool {
    let Some(tail) = marker.trim_start().strip_prefix("| ") else {
        return false;
    };
    let indentation = tail.bytes().take_while(|byte| *byte == b' ').count();
    indentation.checked_add(1) == Some(column)
        && tail
            .get(indentation..)
            .is_some_and(|text| text.starts_with('^'))
}
