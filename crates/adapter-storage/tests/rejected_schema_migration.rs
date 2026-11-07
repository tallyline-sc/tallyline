// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Semantic reader gate for permanent-refusal copy histories.

#[path = "support/schema_fifteen_fixture.rs"]
mod fixture;
#[path = "support/schema_fifteen_reader.rs"]
mod reader;

#[path = "rejected_schema_migration/migration.rs"]
mod migration;
