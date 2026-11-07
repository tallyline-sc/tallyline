// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic, versioned application preferences.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use tallyline_workflows::api::AppSettingsStore;
use tallyline_workflows::api::model::{AppSettings, Theme};

/// Loaded preferences and visible validation failures.
#[derive(Debug)]
pub struct LoadedSettings {
    /// Valid preferences, with invalid values replaced by defaults.
    pub settings: AppSettings,
    /// Settings that require the user's attention.
    pub errors: Vec<SettingsIssue>,
}

/// A rejected persisted preference.
#[derive(Debug, PartialEq, Eq)]
pub struct SettingsIssue {
    /// The invalid setting's catalogue key.
    pub key: &'static str,
    /// The rejected non-secret value.
    pub rejected: String,
    /// The default used instead.
    pub default: &'static str,
}

type MigrationBackup = Box<dyn FnMut(u32, &[u8]) -> io::Result<()> + Send>;

/// The single writer of a versioned settings file.
pub struct ConfigFile {
    path: PathBuf,
    migration_backup: Option<MigrationBackup>,
}

impl ConfigFile {
    /// Creates a store at its resolved configuration path.
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self {
            path,
            migration_backup: None,
        }
    }

    /// Reads exact supported configuration bytes, distinguishing absence from read failure.
    ///
    /// # Errors
    /// Returns malformed JSON, an unsupported schema or a filesystem read failure.
    pub fn backup_source(&self) -> io::Result<Option<crate::api::model::ConfigBackupSource>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let document: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid settings JSON"))?;
        let schema = match document
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
        {
            Some(schema @ 1..=6) => u32::try_from(schema).map_err(io::Error::other)?,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported settings schema version",
                ));
            }
        };
        Ok(Some(crate::api::model::ConfigBackupSource {
            schema,
            bytes,
        }))
    }

    /// Installs the coordinator that backs up exact previous config bytes before migration.
    #[must_use]
    pub fn with_migration_backup(
        mut self,
        backup: impl FnMut(u32, &[u8]) -> io::Result<()> + Send + 'static,
    ) -> Self {
        self.migration_backup = Some(Box::new(backup));
        self
    }

    /// Migrates an existing supported config before normal worker operations.
    ///
    /// # Errors
    /// Returns malformed/future-schema, filesystem or backup errors without replacing old bytes.
    pub fn migrate(&mut self) -> io::Result<()> {
        match fs::metadata(&self.path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
            Ok(_) => (),
        }
        let document = self.read_document()?;
        if document
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            != Some(6)
        {
            self.write_document(document)?;
        }
        Ok(())
    }

    /// Reads versioned preferences and reports rejected values.
    ///
    /// # Errors
    /// Returns filesystem failures or an unsupported future schema version.
    pub fn load(&self) -> io::Result<LoadedSettings> {
        let content = match fs::read(&self.path) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(LoadedSettings {
                    settings: AppSettings::default(),
                    errors: Vec::new(),
                });
            }
            Err(error) => return Err(error),
        };
        let document: serde_json::Value = match serde_json::from_slice(&content) {
            Ok(document) => document,
            Err(_) => {
                return Ok(LoadedSettings {
                    settings: AppSettings::default(),
                    errors: vec![SettingsIssue {
                        key: "settings.document",
                        rejected: "invalid JSON".to_owned(),
                        default: "application defaults",
                    }],
                });
            }
        };
        if !matches!(
            document
                .get("schema_version")
                .and_then(serde_json::Value::as_u64),
            Some(1..=6)
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported settings schema version",
            ));
        }
        let mut errors = Vec::new();
        let theme = match document
            .get("app")
            .and_then(|app| app.get("theme"))
            .and_then(serde_json::Value::as_str)
        {
            Some("system") => Theme::System,
            Some("dark") => Theme::Dark,
            Some("light") => Theme::Light,
            Some("high_contrast") => Theme::HighContrast,
            _ => {
                errors.push(SettingsIssue {
                    key: "settings.theme",
                    rejected: document
                        .get("app")
                        .and_then(|app| app.get("theme"))
                        .map_or_else(|| "missing".to_owned(), serde_json::Value::to_string),
                    default: "system",
                });
                Theme::System
            }
        };
        let onboarding = match document
            .get("app")
            .and_then(|app| app.get("onboarding_complete"))
            .and_then(serde_json::Value::as_bool)
        {
            Some(value) => value,
            None => {
                errors.push(SettingsIssue {
                    key: "settings.onboarding_complete",
                    rejected: document
                        .get("app")
                        .and_then(|app| app.get("onboarding_complete"))
                        .map_or_else(|| "missing".to_owned(), serde_json::Value::to_string),
                    default: "false",
                });
                false
            }
        };
        let (checks, attempt) = crate::update_config::load(&document, &mut errors);
        let (available, dismissed) = crate::update_config::notice(&document, &mut errors);
        let (mode, age) = crate::submission_config::load(&document, &mut errors);
        Ok(LoadedSettings {
            settings: AppSettings::from_persisted(theme, onboarding)
                .with_update_state(checks, attempt)
                .with_release_notice(available, dismissed)
                .with_stock_settings(crate::stock_config::load(&document, &mut errors)?)
                .with_submission_mode(mode)
                .with_observation_age(age),
            errors,
        })
    }
}

impl AppSettingsStore for ConfigFile {
    type Error = io::Error;
    fn save(&mut self, settings: &AppSettings) -> io::Result<()> {
        let mut document = self.read_document()?;
        Self::object(&mut document)?.insert("app".to_owned(), app_document(settings)?);
        self.write_document(document)
    }
}

impl ConfigFile {
    pub(crate) fn object(
        document: &mut serde_json::Value,
    ) -> io::Result<&mut serde_json::Map<String, serde_json::Value>> {
        document.as_object_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "settings document is not an object",
            )
        })
    }
    pub(crate) fn read_document(&self) -> io::Result<serde_json::Value> {
        let mut document: serde_json::Value = match fs::read(&self.path) {
            Ok(content) => serde_json::from_slice(&content)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid settings JSON"))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                serde_json::json!({"schema_version":6,"app":app_document(&AppSettings::default())?})
            }
            Err(error) => return Err(error),
        };
        if !matches!(
            document
                .get("schema_version")
                .and_then(serde_json::Value::as_u64),
            Some(1..=6)
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported settings schema version",
            ));
        }
        migrate_application(&mut document);
        Ok(document)
    }

    pub(crate) fn write_document(&mut self, mut document: serde_json::Value) -> io::Result<()> {
        if let Some(previous @ 1..=5) = document
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
        {
            let backup = self.migration_backup.as_mut().ok_or_else(|| {
                io::Error::other("configuration migration requires a coordinated backup")
            })?;
            backup(
                u32::try_from(previous).map_err(io::Error::other)?,
                &fs::read(&self.path)?,
            )?;
        }
        Self::object(&mut document)?.insert("schema_version".to_owned(), serde_json::json!(6));
        let content = serde_json::to_vec_pretty(&document)?;
        let temporary = self.path.with_extension("json.pending");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| {
            file.write_all(&content)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &self.path)
        })();
        if result.is_err() {
            fs::remove_file(&temporary)?;
        }
        result
    }
}

fn app_document(settings: &AppSettings) -> io::Result<serde_json::Value> {
    let attempt = settings
        .last_update_attempt()
        .map(|time| u64::try_from(time.millis()))
        .transpose()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "release-check time exceeds the settings range",
            )
        })?;
    let theme = match settings.theme() {
        Theme::System => "system",
        Theme::Dark => "dark",
        Theme::Light => "light",
        Theme::HighContrast => "high_contrast",
    };
    Ok(
        serde_json::json!({"submission_mode":settings.submission_mode_override().map(|mode|match mode{tallyline_shared_kernel::api::model::SubmissionMode::DryRun=>"dry_run",tallyline_shared_kernel::api::model::SubmissionMode::Production=>"production"}),"observation_age_ms":settings.observation_age().maximum().as_millis(),"stock_percent":settings.stock_settings().tolerance().percent().get(),"stock_minimum":settings.stock_settings().tolerance().minimum().to_string(),"theme":theme,"onboarding_complete":settings.onboarding_complete(), "update_checks": settings.update_checks() == tallyline_workflows::api::model::UpdateChecks::Enabled, "last_update_attempt_ms": attempt, "available_release": settings.available_release().map(|version| version.to_string()), "dismissed_release": settings.dismissed_release().map(|version| version.to_string())}),
    )
}

fn migrate_application(document: &mut serde_json::Value) {
    let Some(version @ 1..=5) = document
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
    else {
        return;
    };
    if version < 3
        && let Some(app) = document
            .get_mut("app")
            .and_then(serde_json::Value::as_object_mut)
    {
        app.insert("update_checks".to_owned(), serde_json::json!(false));
        for key in [
            "last_update_attempt_ms",
            "available_release",
            "dismissed_release",
        ] {
            app.insert(key.to_owned(), serde_json::Value::Null);
        }
    }
    if version < 5 {
        crate::stock_config::migrate(document);
    }
    crate::submission_config::migrate(document);
}
