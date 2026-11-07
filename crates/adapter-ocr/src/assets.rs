// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed Small model and dictionary integrity at load.

use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::ctc::{CtcDecoder, CtcError};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// One reviewed fixed OCR data component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcrArtifact {
    /// The official Small detector ONNX file.
    SmallDetector,
    /// The official Small recogniser ONNX file.
    SmallRecognizer,
    /// The exact ordered canonical Small dictionary.
    SmallDictionary,
    /// The official Tiny detector.
    TinyDetector,
    /// The official Tiny recogniser.
    TinyRecognizer,
    /// The canonical Tiny dictionary.
    TinyDictionary,
    /// The official Medium detector.
    MediumDetector,
    /// The official Medium recogniser.
    MediumRecognizer,
}

/// The verified public identity of one local OCR data file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetFingerprint {
    /// The model-specific component whose bytes were checked.
    pub artifact: OcrArtifact,
    /// Its reviewed lower-case SHA-256.
    pub sha256: &'static str,
    /// Its exact verified byte count.
    pub bytes: u64,
}

/// Why a fixed OCR data bundle could not be verified.
#[derive(Debug)]
pub enum AssetError {
    /// The caller supplied a relative data path or bundle directory.
    NotAbsolute,
    /// The absolute bundle directory could not be resolved.
    Directory(std::io::Error),
    /// A data file could not be read.
    Read {
        /// The affected fixed component.
        artifact: OcrArtifact,
        /// The underlying filesystem error.
        source: std::io::Error,
    },
    /// A data file's size or digest differs from its reviewed identity.
    HashMismatch(OcrArtifact),
    /// The selected artifact is a dictionary rather than a native model.
    NotModel,
    /// The build did not supply valid fixed asset metadata.
    InvalidBuildPins,
    /// The dictionary bytes are not UTF-8 or have the wrong entry count.
    InvalidDictionary,
    /// The dictionary cannot form the required CTC alphabet.
    Dictionary(CtcError),
}

/// Data identities checked at load and the immutable, model-specific Small CTC decoder.
/// Model files remain external and must be verified again when opened for inference.
pub struct SmallOcrAssets {
    directory: PathBuf,
    fingerprints: [AssetFingerprint; 3],
    decoder: CtcDecoder,
}

#[derive(Clone, Copy)]
pub(crate) struct Pin {
    pub(crate) artifact: OcrArtifact,
    pub(crate) file: &'static str,
    pub(crate) sha256: &'static str,
    pub(crate) bytes: &'static str,
}

pub(crate) const DETECTOR: Pin = Pin {
    artifact: OcrArtifact::SmallDetector,
    file: env!("TALLYLINE_SMALL_DETECTOR_FILE"),
    sha256: env!("TALLYLINE_SMALL_DETECTOR_SHA256"),
    bytes: env!("TALLYLINE_SMALL_DETECTOR_BYTES"),
};
pub(crate) const RECOGNIZER: Pin = Pin {
    artifact: OcrArtifact::SmallRecognizer,
    file: env!("TALLYLINE_SMALL_RECOGNIZER_FILE"),
    sha256: env!("TALLYLINE_SMALL_RECOGNIZER_SHA256"),
    bytes: env!("TALLYLINE_SMALL_RECOGNIZER_BYTES"),
};
pub(crate) const DICTIONARY: Pin = Pin {
    artifact: OcrArtifact::SmallDictionary,
    file: env!("TALLYLINE_SMALL_DICTIONARY_FILE"),
    sha256: env!("TALLYLINE_SMALL_DICTIONARY_SHA256"),
    bytes: env!("TALLYLINE_SMALL_DICTIONARY_BYTES"),
};

impl SmallOcrAssets {
    /// Verifies fixed files and builds the exact nonblank alphabet, with its model space class.
    ///
    /// This blocking call belongs on a bounded worker; it does not initialise a native runtime.
    ///
    /// # Errors
    /// Returns an error for nonabsolute or unreadable paths, changed bytes, invalid build
    /// metadata, or a malformed dictionary; no alternate file or model is selected.
    pub fn load(directory: &Path) -> Result<Self, AssetError> {
        if !directory.is_absolute() {
            return Err(AssetError::NotAbsolute);
        }
        let directory = directory.canonicalize().map_err(AssetError::Directory)?;
        let detector = verify(&directory, DETECTOR, false)?;
        let recognizer = verify(&directory, RECOGNIZER, false)?;
        let dictionary = verify(&directory, DICTIONARY, true)?;
        let contents = dictionary.contents.ok_or(AssetError::InvalidBuildPins)?;
        let text = String::from_utf8(contents).map_err(|_| AssetError::InvalidDictionary)?;
        let mut alphabet: Vec<_> = text.lines().map(str::to_owned).collect();
        let expected: usize = env!("TALLYLINE_SMALL_DICTIONARY_ENTRIES")
            .parse()
            .map_err(|_| AssetError::InvalidBuildPins)?;
        if alphabet.len() != expected {
            return Err(AssetError::InvalidDictionary);
        }
        alphabet.push(" ".to_owned());
        let decoder = CtcDecoder::new(alphabet).map_err(AssetError::Dictionary)?;
        Ok(Self {
            directory,
            fingerprints: [
                detector.fingerprint,
                recognizer.fingerprint,
                dictionary.fingerprint,
            ],
            decoder,
        })
    }

    /// Opens and hashes a fixed model into immutable owned bytes for the native call.
    /// No later model-path reopen is needed or permitted by the runtime.
    ///
    /// # Errors
    /// Rejects the dictionary, changed/unreadable model files or invalid build metadata.
    pub fn open_model(&self, artifact: OcrArtifact) -> Result<VerifiedOcrModel, AssetError> {
        let pin = match artifact {
            OcrArtifact::SmallDetector => DETECTOR,
            OcrArtifact::SmallRecognizer => RECOGNIZER,
            OcrArtifact::SmallDictionary
            | OcrArtifact::TinyDetector
            | OcrArtifact::TinyRecognizer
            | OcrArtifact::TinyDictionary
            | OcrArtifact::MediumDetector
            | OcrArtifact::MediumRecognizer => return Err(AssetError::NotModel),
        };
        let verified = verify(&self.directory, pin, true)?;
        Ok(VerifiedOcrModel {
            fingerprint: verified.fingerprint,
            bytes: verified.contents.ok_or(AssetError::InvalidBuildPins)?,
        })
    }
    /// Returns the absolute directory checked during this load.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Returns the three verified data identities in detector, recogniser, dictionary order.
    #[must_use]
    pub fn fingerprints(&self) -> &[AssetFingerprint] {
        &self.fingerprints
    }

    /// Returns the exact alphabet decoder, including blank and appended space classes.
    #[must_use]
    pub fn decoder(&self) -> &CtcDecoder {
        &self.decoder
    }
}

pub(crate) struct Verified {
    pub(crate) fingerprint: AssetFingerprint,
    pub(crate) contents: Option<Vec<u8>>,
}

fn verify(directory: &Path, pin: Pin, keep_bytes: bool) -> Result<Verified, AssetError> {
    verify_path(&directory.join(pin.file), pin, keep_bytes)
}

pub(crate) fn verify_path(path: &Path, pin: Pin, keep_bytes: bool) -> Result<Verified, AssetError> {
    let bytes: u64 = pin
        .bytes
        .parse()
        .map_err(|_| AssetError::InvalidBuildPins)?;
    let limit = bytes.checked_add(1).ok_or(AssetError::InvalidBuildPins)?;
    let file = std::fs::File::open(path).map_err(|source| AssetError::Read {
        artifact: pin.artifact,
        source,
    })?;
    let metadata = file.metadata().map_err(|source| AssetError::Read {
        artifact: pin.artifact,
        source,
    })?;
    if !metadata.is_file() || metadata.len() != bytes {
        return Err(AssetError::HashMismatch(pin.artifact));
    }
    let mut source = file.take(limit);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut count = 0_u64;
    let mut contents = keep_bytes.then(Vec::new);
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|source| AssetError::Read {
                artifact: pin.artifact,
                source,
            })?;
        if read == 0 {
            break;
        }
        let chunk = buffer.get(..read).ok_or(AssetError::InvalidBuildPins)?;
        digest.update(chunk);
        count += read as u64;
        if let Some(contents) = &mut contents {
            contents.extend_from_slice(chunk);
        }
    }
    let actual: String = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if count != bytes || actual != pin.sha256 {
        return Err(AssetError::HashMismatch(pin.artifact));
    }
    Ok(Verified {
        fingerprint: AssetFingerprint {
            artifact: pin.artifact,
            sha256: pin.sha256,
            bytes,
        },
        contents,
    })
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAbsolute => formatter.write_str("the OCR data directory must be absolute"),
            Self::Directory(source) => {
                write!(formatter, "cannot resolve the OCR data directory: {source}")
            }
            Self::Read { artifact, source } => {
                write!(formatter, "cannot read OCR data {artifact:?}: {source}")
            }
            Self::HashMismatch(artifact) => write!(
                formatter,
                "OCR data {artifact:?} does not match its reviewed identity"
            ),
            Self::NotModel => formatter.write_str("the OCR dictionary is not a native model"),
            Self::InvalidBuildPins => {
                formatter.write_str("the build did not provide valid OCR data identities")
            }
            Self::InvalidDictionary => {
                formatter.write_str("the Small OCR dictionary has invalid encoding or entry count")
            }
            Self::Dictionary(reason) => write!(
                formatter,
                "the Small OCR dictionary cannot be decoded: {reason:?}"
            ),
        }
    }
}

impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Directory(source) | Self::Read { source, .. } => Some(source),
            Self::NotAbsolute
            | Self::NotModel
            | Self::HashMismatch(_)
            | Self::InvalidBuildPins
            | Self::InvalidDictionary
            | Self::Dictionary(_) => None,
        }
    }
}

/// An exact approved model snapshot; its immutable bytes are the native input.
pub struct VerifiedOcrModel {
    pub(crate) fingerprint: AssetFingerprint,
    pub(crate) bytes: Vec<u8>,
}

impl VerifiedOcrModel {
    /// Returns the identity of the same bytes retained during hashing.
    #[must_use]
    pub const fn fingerprint(&self) -> AssetFingerprint {
        self.fingerprint
    }

    /// Borrows the immutable verified ONNX bytes, independent of later path changes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
