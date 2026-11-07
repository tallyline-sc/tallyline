// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Actionable Report-owned findings survive fixable refusals and reopening.

use super::{controller_support, recovery::RecoveryBackend};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tallyline_app::api::HttpDriver;
use tallyline_reporting::api::model::{FixableReason, ReportFinding};
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_submission::api::model::{JobPhase, SubmissionResult, SubmissionReturnReason};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_6_r_sub_11__fixable_refusals_return_exact_values_with_specific_durable_draft_findings() {
    tallyline_app::api::CpuWorker::start("tallyline-fixable-return", || {
        for (outcome, finding) in [
            (
                SubmissionReturnReason::ScreenshotRequired,
                FixableReason::ScreenshotRequired,
            ),
            (
                SubmissionReturnReason::InvalidObservationTime,
                FixableReason::InvalidObservationTime,
            ),
            (
                SubmissionReturnReason::MaxRowsExceeded,
                FixableReason::TooManyRows,
            ),
            (
                SubmissionReturnReason::TerminalNotFound,
                FixableReason::TerminalNotFound,
            ),
            (
                SubmissionReturnReason::InvalidScreenshot,
                FixableReason::InvalidScreenshot,
            ),
            (
                SubmissionReturnReason::ScreenshotTooLarge,
                FixableReason::ScreenshotTooLarge,
            ),
        ] {
            let posts = Arc::new(AtomicUsize::new(0));
            let backend_posts = Arc::clone(&posts);
            let (driver, client) = HttpDriver::start(move || {
                Ok::<_, ()>(RecoveryBackend {
                    posts: backend_posts,
                    result: SubmissionResult::Returned(outcome),
                })
            })
            .unwrap();
            let mut fixture = controller_support::fixture(
                client,
                SubmissionMode::DryRun,
                &format!(
                    "fixable-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ),
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
            while fixture.controller.jobs()[0].phase() != JobPhase::Returned {
                fixture.controller.tick(&fixture.application, None).unwrap();
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            for _ in 0..30 {
                fixture.controller.tick(&fixture.application, None).unwrap();
            }
            fixture.controller.shutdown().unwrap();
            drop(fixture.controller);
            let mut store =
                tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();
            let draft = tallyline_reporting::api::ReportRepository::load_draft(
                &mut store,
                fixture.report.id(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                draft.finding(),
                Some(ReportFinding::ReturnedForFix(finding))
            );
            assert_eq!(draft.rows(), released.report().rows());
            assert_eq!(draft.observed_at(), released.report().observed_at());
            assert_eq!(posts.load(Ordering::SeqCst), 1);
            drop(store);
            driver.finish().unwrap();
            std::fs::remove_file(fixture.path).unwrap();
        }
    })
    .unwrap()
    .finish()
    .unwrap();
}
