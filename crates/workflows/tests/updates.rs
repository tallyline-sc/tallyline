// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Release-check application acceptance through durable file and HTTP boundaries.

use tallyline_adapter_files::api::ConfigFile;
use tallyline_workflows::api::model::{AppSettings, Theme};

struct TestDirectory(std::path::PathBuf);
impl TestDirectory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "tallyline-updates-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TestDirectory {
    #[expect(
        clippy::unwrap_used,
        reason = "Cleanup failure must fail the real-filesystem acceptance test."
    )]
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

struct FixedClock(u64);
impl tallyline_shared_kernel::api::Clock for FixedClock {
    fn now(&self) -> tallyline_shared_kernel::api::model::Timestamp {
        tallyline_shared_kernel::api::model::Timestamp::from_millis(self.0)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_7__daily_attempt_survives_restart_and_other_preference_changes() {
    use tallyline_workflows::api::{SettingsApplication, model::UpdateChecks};
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let mut app = SettingsApplication::new(AppSettings::default(), ConfigFile::new(path.clone()));
    assert!(!app.begin_update_check(&FixedClock(1001)).unwrap());
    app.change_update_checks(UpdateChecks::Enabled).unwrap();
    assert!(app.begin_update_check(&FixedClock(1001)).unwrap());
    app.change_theme(Theme::Light).unwrap();
    app.complete_onboarding().unwrap();
    let store = ConfigFile::new(path);
    let mut app = SettingsApplication::new(store.load().unwrap().settings, store);
    assert!(!app.begin_update_check(&FixedClock(86_401_000)).unwrap());
    assert!(app.begin_update_check(&FixedClock(86_401_001)).unwrap());
    assert!(!app.begin_update_check(&FixedClock(1)).unwrap());
    app.change_update_checks(UpdateChecks::Disabled).unwrap();
    assert!(!app.begin_update_check(&FixedClock(200_000_000)).unwrap());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_7__failed_writes_do_not_enable_or_reserve_release_checks() {
    use tallyline_workflows::api::{SettingsApplication, model::UpdateChecks};
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let pending = path.with_extension("json.pending");
    std::fs::write(&pending, b"blocked").unwrap();
    let mut app = SettingsApplication::new(AppSettings::default(), ConfigFile::new(path.clone()));
    assert!(app.change_update_checks(UpdateChecks::Enabled).is_err());
    assert_eq!(app.settings().update_checks(), UpdateChecks::Disabled);
    assert!(!app.begin_update_check(&FixedClock(1)).unwrap());
    std::fs::remove_file(&pending).unwrap();
    app.change_update_checks(UpdateChecks::Enabled).unwrap();
    std::fs::write(&pending, b"blocked").unwrap();
    assert!(app.begin_update_check(&FixedClock(1)).is_err());
    assert_eq!(app.settings().last_update_attempt(), None);
    assert_eq!(
        ConfigFile::new(path)
            .load()
            .unwrap()
            .settings
            .last_update_attempt(),
        None
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_7_r_ui_16__release_answers_and_network_failures_return_only_passive_state() {
    use tallyline_adapter_platform::api::ReleaseClient;
    use tallyline_workflows::api::{
        check_update,
        model::{ReleaseVersion, UpdateStatus},
    };
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
    let installed = ReleaseVersion::parse("v1.0.0").unwrap();
    for (body, status) in [
        (
            r#"{"tag_name":"v2.0.0","draft":false,"prerelease":false}"#,
            UpdateStatus::Available(ReleaseVersion::parse("v2.0.0").unwrap()),
        ),
        (
            r#"{"tag_name":"v1.0.0","draft":false,"prerelease":false}"#,
            UpdateStatus::Current,
        ),
        (
            r#"{"tag_name":"v0.9.0","draft":false,"prerelease":false}"#,
            UpdateStatus::Current,
        ),
        (
            r#"{"tag_name":"v2.0.0","draft":true,"prerelease":false}"#,
            UpdateStatus::Unavailable,
        ),
        (
            r#"{"tag_name":"v2.0.0","draft":false,"prerelease":true}"#,
            UpdateStatus::Unavailable,
        ),
        (
            r#"{"tag_name":"v2.0.0-rc.1","draft":false,"prerelease":false}"#,
            UpdateStatus::Unavailable,
        ),
        (r#"{"tag_name":"v2.0.0"}"#, UpdateStatus::Unavailable),
        ("not JSON", UpdateStatus::Unavailable),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        let client = ReleaseClient::loopback(server.address().port(), "test").unwrap();
        assert_eq!(check_update(&client, installed).await, status);
    }
    for (code, expected) in [
        (404, UpdateStatus::Current),
        (429, UpdateStatus::Unavailable),
        (500, UpdateStatus::Unavailable),
        (302, UpdateStatus::Unavailable),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(code).insert_header("Location", "https://untrusted.invalid/"),
            )
            .mount(&server)
            .await;
        let client = ReleaseClient::loopback(server.address().port(), "test").unwrap();
        assert_eq!(check_update(&client, installed).await, expected);
    }
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("x".repeat(65_537)))
        .mount(&server)
        .await;
    assert_eq!(
        check_update(
            &ReleaseClient::loopback(server.address().port(), "test").unwrap(),
            installed
        )
        .await,
        UpdateStatus::Unavailable
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    assert_eq!(
        check_update(&ReleaseClient::loopback(port, "test").unwrap(), installed).await,
        UpdateStatus::Unavailable
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_7_r_ui_16__validated_notice_survives_restart_until_dismissed_or_installed() {
    use tallyline_workflows::api::{
        SettingsApplication,
        model::{ReleaseVersion, UpdateChecks, UpdateStatus},
    };
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let installed = ReleaseVersion::parse("v1.0.0").unwrap();
    let newer = ReleaseVersion::parse("v2.0.0").unwrap();
    let mut app = SettingsApplication::new(AppSettings::default(), ConfigFile::new(path.clone()));
    app.change_update_checks(UpdateChecks::Enabled).unwrap();
    app.begin_update_check(&FixedClock(1001)).unwrap();
    app.record_update(UpdateStatus::Available(newer), installed)
        .unwrap();
    app.change_theme(Theme::Light).unwrap();
    app.complete_onboarding().unwrap();
    let store = ConfigFile::new(path.clone());
    let mut app = SettingsApplication::new(store.load().unwrap().settings, store);
    assert_eq!(
        app.settings().cached_update(installed),
        UpdateStatus::Available(newer)
    );
    assert_eq!(app.settings().cached_update(newer), UpdateStatus::Current);
    assert!(!app.begin_update_check(&FixedClock(1002)).unwrap());
    app.record_update(UpdateStatus::Unavailable, installed)
        .unwrap();
    assert_eq!(
        app.settings().cached_update(installed),
        UpdateStatus::Available(newer)
    );
    app.dismiss_update().unwrap();
    let store = ConfigFile::new(path.clone());
    let mut app = SettingsApplication::new(store.load().unwrap().settings, store);
    assert_eq!(
        app.settings().cached_update(installed),
        UpdateStatus::Dismissed
    );
    app.record_update(UpdateStatus::Available(newer), installed)
        .unwrap();
    assert_eq!(
        app.settings().cached_update(installed),
        UpdateStatus::Dismissed
    );
    let newest = ReleaseVersion::parse("v2.0.1").unwrap();
    app.record_update(UpdateStatus::Available(newest), installed)
        .unwrap();
    assert_eq!(
        app.settings().cached_update(installed),
        UpdateStatus::Available(newest)
    );
    app.record_update(UpdateStatus::Current, installed).unwrap();
    let reopened = ConfigFile::new(path).load().unwrap();
    assert_eq!(
        reopened.settings.cached_update(installed),
        UpdateStatus::NotChecked
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_7_r_ui_16__failed_notice_saves_preserve_known_update_and_dismissal_choice() {
    use tallyline_workflows::api::{
        SettingsApplication,
        model::{ReleaseVersion, UpdateChecks, UpdateStatus},
    };
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("settings.json");
    let mut app = SettingsApplication::new(AppSettings::default(), ConfigFile::new(path.clone()));
    let installed = ReleaseVersion::parse("v1.0.0").unwrap();
    let newer = ReleaseVersion::parse("v2.0.0").unwrap();
    app.change_update_checks(UpdateChecks::Enabled).unwrap();
    app.record_update(UpdateStatus::Available(newer), installed)
        .unwrap();
    std::fs::write(path.with_extension("json.pending"), b"blocked").unwrap();
    assert!(app.dismiss_update().is_err());
    assert!(app.record_update(UpdateStatus::Current, installed).is_err());
    assert_eq!(
        app.settings().cached_update(installed),
        UpdateStatus::Available(newer)
    );
    assert_eq!(
        app.settings()
            .resolve_update(installed, UpdateStatus::Unavailable),
        UpdateStatus::Available(newer)
    );
    assert_eq!(
        app.settings()
            .clone()
            .with_update_state(UpdateChecks::Disabled, None)
            .cached_update(installed),
        UpdateStatus::NotChecked
    );
    let loaded = ConfigFile::new(path).load().unwrap();
    assert_eq!(
        loaded.settings.cached_update(installed),
        UpdateStatus::Available(newer)
    );
}
