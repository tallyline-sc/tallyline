// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Manual application behaviour at its public persistence and reference boundaries.

//! Manual application behaviour at its public persistence and reference boundaries.

mod support;

use support::{Fixed, Repository, key, snapshot};
use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
use tallyline_reference_data::api::model::UexEnvironment;
use tallyline_reporting::api::model::{ReportKey, RowId};
use tallyline_reporting::api::{ManualEdit, ReportingApplication, RepositoryError};
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_6__typed_fields_and_takeover_are_durable_user_choices_without_credentials() {
    let snapshot = snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let mut report = app.create(key().unwrap(), Some("4.10.2")).unwrap();
    assert_eq!(report.observed_at(), Timestamp::from_seconds(1000));
    report = app
        .edit(
            report.id(),
            0,
            ManualEdit::AddCommodity(CommodityId(42)),
            &snapshot,
        )
        .unwrap();
    let row = report.rows().first().unwrap().id();
    for edit in [
        ManualEdit::Price(row, Some("999".parse().unwrap())),
        ManualEdit::Stock(row, Some("1.25".parse().unwrap())),
        ManualEdit::Status(row, std::num::NonZeroU8::new(3)),
    ] {
        report = app
            .edit(report.id(), report.version(), edit, &snapshot)
            .unwrap();
    }
    assert_eq!(
        app.drafts()
            .unwrap()
            .first()
            .unwrap()
            .rows()
            .first()
            .unwrap()
            .stock()
            .unwrap()
            .to_string(),
        "1.25"
    );
    report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::TakeOver(row),
            &snapshot,
        )
        .unwrap();
    assert_eq!(
        report.rows().first().unwrap().price().unwrap().to_string(),
        "123.45"
    );
    assert_eq!(
        report.rows().first().unwrap().stock().unwrap().to_string(),
        "0"
    );
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_6__stale_edits_invalid_vocabulary_and_wrong_environment_never_replace_a_draft() {
    let snapshot = snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let report = app.create(key().unwrap(), None).unwrap();
    assert_eq!(
        app.edit(
            report.id(),
            1,
            ManualEdit::AddCommodity(CommodityId(42)),
            &snapshot
        ),
        Err(RepositoryError::Conflict)
    );
    assert_eq!(
        app.edit(
            report.id(),
            0,
            ManualEdit::AddCommodity(CommodityId(999)),
            &snapshot
        ),
        Err(RepositoryError::Invalid)
    );
    let report = app
        .edit(
            report.id(),
            0,
            ManualEdit::AddCommodity(CommodityId(42)),
            &snapshot,
        )
        .unwrap();
    let row = report.rows().first().unwrap().id();
    assert_eq!(
        app.edit(
            report.id(),
            report.version(),
            ManualEdit::Status(row, std::num::NonZeroU8::new(7)),
            &snapshot
        ),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.edit(
            report.id(),
            report.version(),
            ManualEdit::Price(RowId(99), Some("10".parse().unwrap())),
            &snapshot
        ),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_6__changed_side_and_mapping_reassess_status_and_takeover_without_erasing_user_values()
 {
    let live = snapshot(UexEnvironment::Live).unwrap();
    let ptu = snapshot(UexEnvironment::Ptu).unwrap();
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let mut report = app.create(key().unwrap(), Some("4.10.2")).unwrap();
    report = app
        .edit(
            report.id(),
            0,
            ManualEdit::AddCommodity(CommodityId(42)),
            &live,
        )
        .unwrap();
    let row = report.rows().first().unwrap().id();
    report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::TakeOver(row),
            &live,
        )
        .unwrap();
    report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::ChangeKey(
                ReportKey::new(TerminalId(11), TradeSide::Sell, GameEnvironment::Live).unwrap(),
            ),
            &live,
        )
        .unwrap();
    assert!(
        app.edit(
            report.id(),
            report.version(),
            ManualEdit::TakeOver(row),
            &live
        )
        .is_err()
    );
    assert!(
        app.edit(
            report.id(),
            report.version(),
            ManualEdit::Status(row, std::num::NonZeroU8::new(3)),
            &live
        )
        .is_err()
    );
    app.set_mapping(
        EnvironmentMapping::default().with_environment(GameEnvironment::Live, UexEnvironment::Ptu),
    );
    assert!(
        app.edit(
            report.id(),
            report.version(),
            ManualEdit::Price(row, None),
            &live
        )
        .is_err()
    );
    report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::Price(row, None),
            &ptu,
        )
        .unwrap();
    assert_eq!(report.rows().first().unwrap().price(), None);
    assert_eq!(
        report.rows().first().unwrap().stock().unwrap().to_string(),
        "0"
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_6__creating_from_reference_shows_known_commodity_inputs_empty_with_the_current_version()
 {
    let snapshot = snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let report = app
        .create_from_reference(key().unwrap(), &snapshot)
        .unwrap();
    assert_eq!(report.game_version(), Some("4.10.2"));
    assert_eq!(report.rows().len(), 1);
    let row = report.rows().first().unwrap();
    assert_eq!(row.commodity(), CommodityId(42));
    assert_eq!(row.price(), None);
    assert_eq!(row.stock(), None);
    assert_eq!(row.status(), None);
    assert_eq!(row.origin(), None);
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_6__reselecting_the_current_report_key_keeps_the_draft_available_without_a_false_error()
 {
    let snapshot = snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let report = app.create(key().unwrap(), Some("4.10.2")).unwrap();
    assert_eq!(
        app.edit(
            report.id(),
            report.version(),
            ManualEdit::ChangeKey(key().unwrap()),
            &snapshot
        )
        .unwrap(),
        report
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_1_r_man_6__creation_refuses_each_invalid_context_without_persisting_a_draft() {
    let live = snapshot(UexEnvironment::Live).unwrap();
    let ptu = snapshot(UexEnvironment::Ptu).unwrap();
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    for (key, reference) in [
        (key().unwrap(), &ptu),
        (
            ReportKey::new(TerminalId(99), TradeSide::Buy, GameEnvironment::Live).unwrap(),
            &live,
        ),
    ] {
        assert_eq!(
            app.create_from_reference(key, reference),
            Err(RepositoryError::Invalid)
        );
        assert!(app.drafts().unwrap().is_empty());
    }
    let report = app
        .create_from_reference(
            ReportKey::new(TerminalId(11), TradeSide::Sell, GameEnvironment::Live).unwrap(),
            &live,
        )
        .unwrap();
    assert!(report.rows().is_empty());
    assert_eq!(report.version(), 0);
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7_q_manual_r_val_6_q_manual__preview_and_time_decisions_belong_to_the_current_attachment()
 {
    use tallyline_reporting::api::AttachmentEdit;
    use tallyline_reporting::api::model::{
        AttachmentBinding, AttachmentBlock, AttachmentRegion, AttachmentTime, ImageHash,
    };
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let report = app.create(key().unwrap(), Some("4.10.2")).unwrap();
    let binding = AttachmentBinding::new(
        ImageHash::new([1; 32]),
        AttachmentRegion::new(10, 20, 30, 40).unwrap(),
    );
    let selected = app
        .edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::Select(
                binding,
                AttachmentTime::ImportedFile(Timestamp::from_seconds(900)),
            ),
        )
        .unwrap();
    assert_eq!(
        selected.attachment_gate(),
        Err(AttachmentBlock::PreviewUnconfirmed)
    );
    assert_eq!(
        app.edit_attachment(
            selected.id(),
            report.version(),
            AttachmentEdit::ConfirmPreview(binding)
        ),
        Err(RepositoryError::Conflict)
    );
    let approved = app
        .edit_attachment(
            selected.id(),
            selected.version(),
            AttachmentEdit::ConfirmPreview(binding),
        )
        .unwrap();
    assert_eq!(
        approved.attachment_gate(),
        Err(AttachmentBlock::TimeUncertain)
    );
    assert_eq!(
        app.edit_attachment(
            approved.id(),
            approved.version(),
            AttachmentEdit::ConfirmTime(Timestamp::from_seconds(901))
        ),
        Err(RepositoryError::Invalid)
    );
    let confirmed = app
        .edit_attachment(
            approved.id(),
            approved.version(),
            AttachmentEdit::ConfirmTime(Timestamp::from_seconds(899)),
        )
        .unwrap();
    assert_eq!(confirmed.observed_at(), Timestamp::from_seconds(899));
    assert_eq!(confirmed.attachment_gate(), Ok(()));
    let unchanged = app
        .edit_attachment(
            confirmed.id(),
            confirmed.version(),
            AttachmentEdit::Select(
                binding,
                AttachmentTime::ImportedFile(Timestamp::from_seconds(999)),
            ),
        )
        .unwrap();
    assert_eq!(unchanged, confirmed);
    let replacement = AttachmentBinding::new(ImageHash::new([2; 32]), binding.region());
    let replaced = app
        .edit_attachment(
            confirmed.id(),
            confirmed.version(),
            AttachmentEdit::Select(
                replacement,
                AttachmentTime::ImportedFile(Timestamp::from_seconds(950)),
            ),
        )
        .unwrap();
    assert_eq!(
        replaced.attachment_gate(),
        Err(AttachmentBlock::PreviewUnconfirmed)
    );
    assert_eq!(replaced.observed_at(), Timestamp::from_seconds(899));
    assert_eq!(
        app.edit_attachment(
            replaced.id(),
            replaced.version(),
            AttachmentEdit::ConfirmTime(Timestamp::from_seconds(900))
        ),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.edit_attachment(
            replaced.id(),
            replaced.version(),
            AttachmentEdit::ConfirmPreview(binding)
        ),
        Err(RepositoryError::Invalid)
    );
    let removed = app
        .edit_attachment(replaced.id(), replaced.version(), AttachmentEdit::Remove)
        .unwrap();
    assert_eq!(removed.observed_at(), Timestamp::from_seconds(899));
    assert_eq!(removed.attachment_gate(), Ok(()));
    assert_eq!(
        app.edit_attachment(
            removed.id(),
            removed.version(),
            AttachmentEdit::ConfirmPreview(replacement)
        ),
        Err(RepositoryError::Invalid)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7__confirmed_working_copy_is_bound_to_current_report_and_selection() {
    use tallyline_reporting::api::AttachmentEdit;
    use tallyline_reporting::api::model::{
        AttachmentBinding, AttachmentRegion, AttachmentTime, ImageHash,
    };
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let mut report = app.create(key().unwrap(), Some("4.10.2")).unwrap();
    let binding = AttachmentBinding::new(
        ImageHash::new([1; 32]),
        AttachmentRegion::new(0, 10, 20, 30).unwrap(),
    );
    report = app
        .edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::Select(
                binding,
                AttachmentTime::ImportedFile(Timestamp::from_seconds(900)),
            ),
        )
        .unwrap();
    let digest = ImageHash::new([2; 32]);
    assert_eq!(
        app.edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::BindWorkingCopy(binding, digest)
        ),
        Err(RepositoryError::Invalid)
    );
    let before = report.version();
    report = app
        .confirm_attachment_copy(report.id(), before, binding, digest)
        .unwrap();
    assert_eq!(
        app.drafts().unwrap()[0]
            .attachment()
            .unwrap()
            .working_copy(),
        Some(digest)
    );
    assert_eq!(
        app.edit_attachment(
            report.id(),
            before,
            AttachmentEdit::BindWorkingCopy(binding, digest)
        ),
        Err(RepositoryError::Conflict)
    );
    let replacement = AttachmentBinding::new(ImageHash::new([3; 32]), binding.region());
    report = app
        .edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::Select(
                replacement,
                AttachmentTime::ImportedFile(Timestamp::from_seconds(950)),
            ),
        )
        .unwrap();
    assert_eq!(report.attachment().unwrap().working_copy(), None);
    assert_eq!(
        app.confirm_attachment_copy(report.id(), before, binding, digest),
        Err(RepositoryError::Conflict)
    );

    assert_eq!(
        app.edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::BindWorkingCopy(binding, digest)
        ),
        Err(RepositoryError::Invalid)
    );
    report = app
        .edit_attachment(report.id(), report.version(), AttachmentEdit::Remove)
        .unwrap();
    assert_eq!(report.observed_at(), Timestamp::from_seconds(900));
    assert_eq!(
        app.edit_attachment(
            report.id(),
            report.version(),
            AttachmentEdit::BindWorkingCopy(binding, digest)
        ),
        Err(RepositoryError::Invalid)
    );
}
