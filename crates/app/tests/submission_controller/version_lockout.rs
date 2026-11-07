// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit newer-release recovery of a durable whole-queue version lockout.

use super::{controller_support, recovery::RecoveryBackend, support};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tallyline_app::api::HttpDriver;
use tallyline_app::api::model::{BuildInformation, BuildTarget};
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_submission::api::model::{JobPhase, SubmissionPauseReason, SubmissionResult};

fn build(
    tag: Option<&'static str>,
) -> Result<BuildInformation, tallyline_app::api::model::BuildInformationError> {
    BuildInformation::new(
        tag,
        tag.map(|_| "1234567890123456789012345678901234567890"),
        BuildTarget::WindowsX64,
    )
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11_r_api_7__only_an_explicit_newer_compiled_release_acknowledges_a_cancelled_version_lockout()
 {
    tallyline_app::api::CpuWorker::start("tallyline-version-recovery", || {
        for (origin, current, allowed, cancel) in [
            (Some("v1.2.0"), Some("v1.2.0"), false, true),
            (Some("v1.2.0"), Some("v1.1.9"), false, true),
            (Some("v1.2.0"), None, false, true),
            (None, Some("v1.2.1"), false, true),
            (Some("v1.2.0"), Some("v1.2.1"), true, true),
            (Some("v1.2.0"), Some("v1.2.1"), true, false),
            (Some("v1.2.0"), Some("v1.2.0"), false, false),
        ] {
            let posts = Arc::new(AtomicUsize::new(0));
            let backend_posts = Arc::clone(&posts);
            let (driver, client) = HttpDriver::start(move || {
                Ok::<_, ()>(RecoveryBackend {
                    posts: backend_posts,
                    result: SubmissionResult::Paused(SubmissionPauseReason::UpdateRequired),
                })
            })
            .unwrap();
            let mut fixture = controller_support::fixture(
                client.clone(),
                SubmissionMode::DryRun,
                &format!(
                    "version-lock-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ),
            )
            .unwrap();
            fixture.controller.shutdown().unwrap();
            drop(fixture.controller);
            fixture.controller = controller_support::open_with_build_information(
                &fixture.path,
                client.clone(),
                build(origin).unwrap(),
            )
            .unwrap();
            let released = fixture
                .controller
                .release_manual(
                    fixture.report.id(),
                    fixture.report.version(),
                    SubmissionMode::DryRun,
                    &fixture.application,
                )
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while fixture.controller.jobs()[0].phase() != JobPhase::Paused {
                fixture.controller.tick(&fixture.application, None).unwrap();
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            let source = if cancel {
                fixture
                    .controller
                    .cancel(released.job().id(), fixture.controller.jobs()[0].revision())
                    .unwrap()
            } else {
                fixture.controller.jobs()[0].clone()
            };
            fixture.controller.shutdown().unwrap();
            drop(fixture.controller);
            fixture.controller = controller_support::open_with_build_information(
                &fixture.path,
                client,
                build(current).unwrap(),
            )
            .unwrap();
            let source = fixture
                .controller
                .jobs()
                .iter()
                .find(|job| job.id() == source.id())
                .unwrap()
                .clone();
            let (store, second) = support::stored_gate(&fixture.path).unwrap();
            drop(store);
            fixture
                .controller
                .release_manual(
                    second.source().id(),
                    second.source().version(),
                    SubmissionMode::DryRun,
                    &fixture.application,
                )
                .unwrap();
            for _ in 0..30 {
                fixture.controller.tick(&fixture.application, None).unwrap();
            }
            assert_eq!(posts.load(Ordering::SeqCst), 1);
            assert_eq!(
                fixture
                    .controller
                    .retry_now(source.id(), source.revision())
                    .is_ok(),
                allowed
            );
            if allowed {
                while posts.load(Ordering::SeqCst) < 2 {
                    fixture.controller.tick(&fixture.application, None).unwrap();
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                }
            } else {
                for _ in 0..30 {
                    fixture.controller.tick(&fixture.application, None).unwrap();
                }
            }
            let original = fixture
                .controller
                .jobs()
                .iter()
                .find(|job| job.id() == source.id())
                .unwrap();
            assert_eq!(
                original.phase(),
                if cancel {
                    JobPhase::Cancelled
                } else {
                    JobPhase::Returned
                }
            );
            assert_eq!(original.attempts().len(), 1);
            assert_eq!(posts.load(Ordering::SeqCst), if allowed { 2 } else { 1 });
            fixture.controller.shutdown().unwrap();
            drop(fixture.controller);
            let mut store =
                tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();
            let original =
                tallyline_submission::api::SubmissionJobRepository::load(&mut store, source.id())
                    .unwrap()
                    .unwrap();
            assert_eq!(
                original.phase(),
                if cancel {
                    JobPhase::Cancelled
                } else {
                    JobPhase::Returned
                }
            );
            assert_eq!(original.attempts().len(), 1);
            drop(store);
            driver.finish().unwrap();
            std::fs::remove_file(fixture.path).unwrap();
        }
    })
    .unwrap()
    .finish()
    .unwrap();
}
