-- SPDX-FileCopyrightText: 2026 Lucas Greuloch
-- SPDX-License-Identifier: GPL-3.0-or-later

CREATE TABLE submission_service_fences(cause TEXT NOT NULL CHECK(cause IN('api_changed','daily_quota','account_pause')),source_job INTEGER NOT NULL REFERENCES submission_jobs(id),source_revision INTEGER NOT NULL CHECK(source_revision>0),PRIMARY KEY(cause,source_job));
PRAGMA user_version=10;
