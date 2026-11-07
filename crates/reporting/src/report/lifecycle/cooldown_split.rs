// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Value-preserving released Report partition with fresh owning-job bindings.

use super::{
    InvalidReport, Report, ReportChange, ReportEventKind, ReportReleaseBinding, ReportState,
};
use tallyline_shared_kernel::api::model::{ReportId, RowId};

/// Two disjoint complete released row groups retaining their exact review decisions.
///
/// ```compile_fail
/// use tallyline_reporting::api::model::ReportSplit;
/// fn repeat_group(split: &mut ReportSplit) {
///     split.cooling = split.ready.clone();
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportSplit {
    /// The original Report identity retaining only the cooling rows.
    cooling: ReportChange,
    /// A separate Report identity retaining only the remaining rows.
    ready: ReportChange,
}
impl ReportSplit {
    /// Borrows the retained Report identity and its cooling-row release events.
    #[must_use]
    pub const fn cooling(&self) -> &ReportChange {
        &self.cooling
    }

    /// Borrows the separate Report identity and its remaining-row release events.
    #[must_use]
    pub const fn ready(&self) -> &ReportChange {
        &self.ready
    }
}
impl Report {
    /// Partitions queued immutable values without returning either group to Draft.
    ///
    /// # Errors
    /// Refuses foreign, repeated, empty or complete row selections and reused identities.
    /// Workflows must establish the current mixed cooldown and cancel the owning job atomically.
    pub fn split_off_cooldown_rows(
        &self,
        rows: &[RowId],
        ready_id: ReportId,
    ) -> Result<ReportSplit, InvalidReport> {
        if !matches!(
            self.state(),
            ReportState::Queued | ReportState::WaitingForCooldown(_)
        ) || ready_id.0 == 0
            || ready_id == self.id()
            || rows.is_empty()
            || rows.len() >= self.rows().iter().filter(|row| row.entered()).count()
        {
            return Err(InvalidReport);
        }
        let unique = rows
            .iter()
            .map(|row| row.0)
            .collect::<std::collections::BTreeSet<_>>();
        if unique.len() != rows.len()
            || rows.iter().any(|id| {
                !self
                    .rows()
                    .iter()
                    .any(|row| row.id() == *id && row.entered())
            })
        {
            return Err(InvalidReport);
        }
        let binding = self.release_binding().ok_or(InvalidReport)?;
        let mut cooling = self.advanced()?;
        let mut ready = self.clone();
        ready.id = ready_id;
        ready.version = 1;
        cooling
            .rows
            .retain(|row| !row.entered() || unique.contains(&row.id().0));
        ready
            .rows
            .retain(|row| row.entered() && !unique.contains(&row.id().0));
        for report in [&mut cooling, &mut ready] {
            report.lifecycle.state = ReportState::Released;
            report.lifecycle.finding = None;
            report.lifecycle.release = Some(ReportReleaseBinding::new(
                binding.account().to_owned(),
                binding.mode(),
                binding.environment(),
                report.version(),
            )?);
            report
                .lifecycle
                .newer_observations
                .retain(|approval| report.rows.iter().any(|row| row.id() == approval.row()));
        }
        Ok(ReportSplit {
            cooling: ReportChange::new(self.version(), cooling, Some(ReportEventKind::Released))?,
            ready: ReportChange::new(0, ready, Some(ReportEventKind::Released))?,
        })
    }
}
