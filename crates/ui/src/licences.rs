// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated offline component metadata and unmodified notice texts.

use serde::Deserialize;

/// One bundled licence or notice file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenceNotice {
    /// Relative location in the bundled legal directory.
    pub path: String,
    /// Unmodified file contents.
    pub text: String,
}

/// Source-backed copyright notices or a reviewed absence of a named holder.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CopyrightInformation {
    /// The upstream materials provide these verbatim attribution lines.
    Provided {
        /// Unmodified copyright-holder lines.
        notices: Vec<String>,
    },
    /// The exact reviewed upstream version names no holder.
    NotStatedByUpstream,
}

/// A shipped component with source attribution and all bundled notices.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenceComponent {
    /// The component name.
    pub name: String,
    /// The exact shipped version.
    pub version: String,
    /// The declared or elected SPDX expression.
    pub licence: String,
    /// The unmodified upstream manifest expression, when the component has one.
    #[serde(default)]
    pub declared_licence: Option<String>,
    /// The source-code URL.
    pub source: String,
    /// Verbatim copyright-holder lines.
    pub copyright: CopyrightInformation,
    /// Full bundled licence and notice files.
    pub notices: Vec<LicenceNotice>,
}

/// The generated catalogue of components in the shipped dependency graph.
#[derive(Debug)]
pub struct LicenceCatalogue {
    components: Vec<LicenceComponent>,
}

impl LicenceCatalogue {
    /// Returns every shipped component.
    #[must_use]
    pub fn components(&self) -> &[LicenceComponent] {
        &self.components
    }
}

/// A catalogue that cannot safely be presented as complete.
#[derive(Debug, PartialEq, Eq)]
pub enum LicenceCatalogueError {
    /// The document is not valid catalogue JSON.
    InvalidJson,
    /// Its schema is not understood by this build.
    UnsupportedSchema,
    /// It names no shipped component.
    EmptyCatalogue,
    /// A component lacks source, attribution or readable notice text.
    MissingMetadata,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    components: Vec<LicenceComponent>,
}

/// Parses a complete generated catalogue without performing I/O or opening links.
///
/// # Errors
/// Returns invalid JSON, unsupported schema or missing component metadata.
pub fn parse_licence_catalogue(text: &str) -> Result<LicenceCatalogue, LicenceCatalogueError> {
    let document: Document =
        serde_json::from_str(text).map_err(|_| LicenceCatalogueError::InvalidJson)?;
    if document.schema_version != 1 {
        return Err(LicenceCatalogueError::UnsupportedSchema);
    }
    if document.components.is_empty() {
        return Err(LicenceCatalogueError::EmptyCatalogue);
    }
    for component in &document.components {
        if [
            &component.name,
            &component.version,
            &component.licence,
            &component.source,
        ]
        .iter()
        .any(|text| text.trim().is_empty())
            || !(component.source.starts_with("https://")
                || component.source.starts_with("http://"))
            || matches!(&component.copyright, CopyrightInformation::Provided { notices } if notices.is_empty() || notices.iter().any(|line| line.trim().is_empty()))
            || component
                .declared_licence
                .as_ref()
                .is_some_and(|text| text.trim().is_empty())
            || component.notices.is_empty()
            || component
                .notices
                .iter()
                .any(|notice| notice.path.trim().is_empty() || notice.text.trim().is_empty())
        {
            return Err(LicenceCatalogueError::MissingMetadata);
        }
    }
    Ok(LicenceCatalogue {
        components: document.components,
    })
}
