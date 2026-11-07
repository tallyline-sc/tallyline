// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Model-specific CTC decoding.

use std::ops::Range;

/// A CTC decoder whose ordered alphabet excludes the blank class at index zero.
#[derive(Clone, Debug)]
pub struct CtcDecoder {
    alphabet: Vec<String>,
}

/// A decoded line with the evidence for each emitted character.
#[derive(Clone, Debug, PartialEq)]
pub struct CtcReading {
    /// The concatenated model labels, without blanks or collapsed repetitions.
    pub text: String,
    /// Emitted characters in frame order.
    pub characters: Vec<CtcCharacter>,
}

/// One emitted label and its contiguous emitting frame span.
#[derive(Clone, Debug, PartialEq)]
pub struct CtcCharacter {
    /// The exact dictionary label.
    pub symbol: String,
    /// The maximum model probability among its emitting frames.
    pub probability: f32,
    /// The half-open frame range before any coordinate mapping.
    pub frames: Range<usize>,
}

/// Invalid alphabet, tensor shape or probability data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtcError {
    /// Labels must be nonempty and the alphabet bounded to 65,535 entries.
    InvalidAlphabet,
    /// Batch size, class count or element count does not match the alphabet.
    InvalidShape,
    /// A probability is nonfinite or outside the closed interval zero to one.
    InvalidProbability,
}

impl CtcDecoder {
    /// Returns the model class count, including blank zero.
    #[must_use]
    pub fn class_count(&self) -> usize {
        self.alphabet.len() + 1
    }

    /// Creates a decoder from every nonblank model label, including space if present.
    ///
    /// # Errors
    /// Returns `InvalidAlphabet` for empty labels or an empty or oversized alphabet.
    pub fn new(alphabet: Vec<String>) -> Result<Self, CtcError> {
        if alphabet.is_empty()
            || alphabet.len() > u16::MAX as usize
            || alphabet.iter().any(String::is_empty)
        {
            return Err(CtcError::InvalidAlphabet);
        }
        Ok(Self { alphabet })
    }

    /// Collapses equal adjacent classes and removes blank zero from probability tensors.
    ///
    /// # Errors
    /// Returns an error for a shape other than `[1, frames, alphabet + 1]`, a missing
    /// element, or a nonfinite or out-of-range probability.
    pub fn decode(&self, probabilities: &[f32], shape: [usize; 3]) -> Result<CtcReading, CtcError> {
        let [batch, frames, classes] = shape;
        if batch != 1
            || frames == 0
            || classes != self.alphabet.len() + 1
            || frames.checked_mul(classes) != Some(probabilities.len())
        {
            return Err(CtcError::InvalidShape);
        }
        if probabilities
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(CtcError::InvalidProbability);
        }
        let mut characters: Vec<CtcCharacter> = Vec::new();
        let mut previous = 0;
        for (frame, row) in probabilities.chunks_exact(classes).enumerate() {
            let (class, probability) =
                row.iter()
                    .copied()
                    .enumerate()
                    .fold((0, f32::NEG_INFINITY), |best, next| {
                        if next.1 > best.1 { next } else { best }
                    });
            if class != 0 {
                if class == previous {
                    if let Some(character) = characters.last_mut() {
                        character.probability = character.probability.max(probability);
                        character.frames.end = frame + 1;
                    }
                } else {
                    let symbol = self
                        .alphabet
                        .get(class - 1)
                        .ok_or(CtcError::InvalidShape)?
                        .clone();
                    characters.push(CtcCharacter {
                        symbol,
                        probability,
                        frames: frame..frame + 1,
                    });
                }
            }
            previous = class;
        }
        let text = characters
            .iter()
            .map(|character| character.symbol.as_str())
            .collect();
        Ok(CtcReading { text, characters })
    }
}
