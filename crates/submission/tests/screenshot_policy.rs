// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Account-scoped dated screenshot evidence at the Submission application seam.

use std::collections::BTreeMap;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{
    AccountState, Persistence, ScreenshotPolicy, ScreenshotRequirement, StorageFailure,
};
use tallyline_submission::api::{ScreenshotPolicyApplication, ScreenshotPolicyStore};

#[derive(Default)]
struct Store {
    values: BTreeMap<String, Timestamp>,
    fail: bool,
}
impl ScreenshotPolicyStore for Store {
    fn required_since(&mut self, account: &str) -> Result<Option<Timestamp>, StorageFailure> {
        Ok(self.values.get(account).copied())
    }
    fn remember(&mut self, account: &str, at: Timestamp) -> Result<(), StorageFailure> {
        if self.fail {
            return Err(StorageFailure::WriteFailed);
        }
        let value = self.values.entry(account.to_owned()).or_insert(at);
        *value = (*value).max(at);
        Ok(())
    }
    fn clear(&mut self, account: &str) -> Result<(), StorageFailure> {
        if self.fail {
            return Err(StorageFailure::WriteFailed);
        }
        self.values.remove(account);
        Ok(())
    }
}
fn account(name: &str) -> AccountState {
    AccountState::Validated {
        username: name.to_owned(),
        validated_at: Timestamp::from_seconds(10),
        persistence: Persistence::Session,
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7__screenshot_requirement_is_dated_scoped_and_explicitly_clearable() {
    let mut app = ScreenshotPolicyApplication::new(Store::default());
    assert_eq!(
        app.current(&account("dummy-a")).unwrap(),
        ScreenshotPolicy::Unknown
    );
    app.observe(
        &account("dummy-a"),
        ScreenshotRequirement::Required,
        Timestamp::from_seconds(100),
    )
    .unwrap();
    app.observe(
        &account("dummy-a"),
        ScreenshotRequirement::Required,
        Timestamp::from_seconds(90),
    )
    .unwrap();
    assert_eq!(
        app.current(&account("dummy-a")).unwrap(),
        ScreenshotPolicy::Required {
            observed_at: Timestamp::from_seconds(100)
        }
    );
    assert_eq!(
        app.current(&account("dummy-b")).unwrap(),
        ScreenshotPolicy::Unknown
    );
    assert_eq!(
        app.current(&AccountState::Unconfigured).unwrap(),
        ScreenshotPolicy::Unknown
    );
    assert_eq!(
        app.observe(
            &AccountState::Unconfigured,
            ScreenshotRequirement::Required,
            Timestamp::from_seconds(101)
        ),
        Err(StorageFailure::Unavailable)
    );
    app.try_without_screenshot(&account("dummy-b")).unwrap();
    assert!(matches!(
        app.current(&account("dummy-a")).unwrap(),
        ScreenshotPolicy::Required { .. }
    ));
    app.try_without_screenshot(&account("dummy-a")).unwrap();
    assert_eq!(
        app.current(&account("dummy-a")).unwrap(),
        ScreenshotPolicy::Unknown
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7__failed_policy_writes_do_not_claim_a_clear_or_an_observation() {
    let mut values = BTreeMap::new();
    values.insert("dummy-a".to_owned(), Timestamp::from_seconds(100));
    let mut app = ScreenshotPolicyApplication::new(Store { values, fail: true });
    assert_eq!(
        app.try_without_screenshot(&account("dummy-a")),
        Err(StorageFailure::WriteFailed)
    );
    assert_eq!(
        app.observe(
            &account("dummy-b"),
            ScreenshotRequirement::Required,
            Timestamp::from_seconds(101)
        ),
        Err(StorageFailure::WriteFailed)
    );
    assert_eq!(
        app.current(&account("dummy-a")).unwrap(),
        ScreenshotPolicy::Required {
            observed_at: Timestamp::from_seconds(100)
        }
    );
    assert_eq!(
        app.current(&account("dummy-b")).unwrap(),
        ScreenshotPolicy::Unknown
    );
}
