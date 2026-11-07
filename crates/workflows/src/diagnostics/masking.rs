// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Overlapping private-value matching with original UTF-8 boundaries.

use std::ops::Range;

const REDACTED: &str = "[redacted]";

pub(super) struct MaskingRule {
    key: String,
    prefix: Vec<usize>,
}

impl MaskingRule {
    fn new(key: String) -> Self {
        let mut prefix = vec![0];
        let mut matched = 0;
        for byte in key.bytes().skip(1) {
            matched = advance(key.as_bytes(), &prefix, matched, byte);
            prefix.push(matched);
        }
        Self { key, prefix }
    }

    fn collect_ranges(
        &self,
        input: &str,
        original: &[(usize, usize)],
        ranges: &mut Vec<(usize, usize)>,
    ) {
        let mut matched = 0;
        for (index, byte) in input.bytes().enumerate() {
            matched = advance(self.key.as_bytes(), &self.prefix, matched, byte);
            if matched == self.key.len() {
                let start = index + 1 - matched;
                if let (Some(first), Some(last)) = (original.get(start), original.get(index)) {
                    ranges.push((first.0, last.1));
                }
                matched = self.prefix.last().copied().unwrap_or_default();
            }
        }
    }
}

pub(super) fn prepare_rules<'a>(values: impl Iterator<Item = &'a str>) -> Vec<MaskingRule> {
    let mut keys: Vec<String> = values
        .map(|value| value.chars().flat_map(matching_char).collect())
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys.into_iter().map(MaskingRule::new).collect()
}

pub(super) fn redact(input: &str, rules: &[MaskingRule]) -> String {
    let mut ranges = matching_ranges(input, rules);
    ranges.sort_unstable();
    let mut output = String::new();
    let mut cursor = 0;
    for (start, end) in ranges {
        if start >= cursor {
            if let Some(prefix) = input.get(cursor..start) {
                output.push_str(prefix);
            }
            output.push_str(REDACTED);
        }
        cursor = cursor.max(end);
    }
    if let Some(suffix) = input.get(cursor..) {
        output.push_str(suffix);
    }
    output
}

pub(super) fn crosses_removed_boundary(
    input: &str,
    rules: &[MaskingRule],
    removed: &[Range<usize>],
) -> bool {
    if removed.is_empty() {
        return false;
    }
    matching_ranges(input, rules)
        .into_iter()
        .any(|(start, end)| {
            removed.iter().any(|range| {
                start < range.end
                    && range.start < end
                    && !(range.start <= start && end <= range.end)
            })
        })
}

fn matching_ranges(input: &str, rules: &[MaskingRule]) -> Vec<(usize, usize)> {
    if rules.is_empty() {
        return Vec::new();
    }
    let mut key = String::new();
    let mut original = Vec::new();
    for (start, character) in input.char_indices() {
        let end = start + character.len_utf8();
        for folded in matching_char(character) {
            key.push(folded);
            original.extend(std::iter::repeat_n((start, end), folded.len_utf8()));
        }
    }
    let mut ranges = Vec::new();
    for rule in rules {
        rule.collect_ranges(&key, &original, &mut ranges);
    }
    ranges
}

fn matching_char(character: char) -> impl Iterator<Item = char> {
    character.to_lowercase().flat_map(char::to_uppercase)
}

fn advance(key: &[u8], prefix: &[usize], mut matched: usize, byte: u8) -> usize {
    let fallback_steps = matched;
    for _ in 0..fallback_steps {
        if matched == 0 || key.get(matched) == Some(&byte) {
            break;
        }
        matched = prefix.get(matched - 1).copied().unwrap_or_default();
    }
    if key.get(matched) == Some(&byte) {
        matched += 1;
    }
    matched
}
