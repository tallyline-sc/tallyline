-- SPDX-FileCopyrightText: 2026 Lucas Greuloch
-- SPDX-License-Identifier: GPL-3.0-or-later

CREATE TABLE ref_cache(endpoint TEXT PRIMARY KEY, fetched_at TEXT NOT NULL, payload TEXT NOT NULL);
CREATE TABLE ref_commodities(id INTEGER PRIMARY KEY, name TEXT NOT NULL, code TEXT NOT NULL, available_live INTEGER NOT NULL);
CREATE INDEX ref_commodities_name ON ref_commodities(name);
CREATE INDEX ref_commodities_code ON ref_commodities(code);
CREATE TABLE ref_terminals(id INTEGER PRIMARY KEY,name TEXT NOT NULL,nickname TEXT NOT NULL,display_name TEXT NOT NULL,location TEXT NOT NULL,available_live INTEGER NOT NULL,max_container_size INTEGER CHECK(max_container_size IS NULL OR max_container_size>0));
CREATE INDEX ref_terminals_name ON ref_terminals(name);
CREATE INDEX ref_terminals_nickname ON ref_terminals(nickname);
CREATE INDEX ref_terminals_display_name ON ref_terminals(display_name);
CREATE INDEX ref_terminals_location ON ref_terminals(location);
CREATE TABLE reports(id INTEGER PRIMARY KEY AUTOINCREMENT,version INTEGER NOT NULL CHECK(version>=0),payload TEXT NOT NULL);
CREATE TABLE report_recents(report_id INTEGER PRIMARY KEY REFERENCES reports(id),changed_order INTEGER NOT NULL);
CREATE TABLE screenshot_requirements(account_digest BLOB PRIMARY KEY CHECK(length(account_digest)=32),observed_at TEXT NOT NULL);
INSERT INTO reports VALUES(1,2,'{"terminal":11,"side":"buy","environment":"live","observed_at":"1000000","game_version":"4.10.2","rows":[{"id":1,"commodity":42,"price":null,"stock":"1744","status":null,"price_decision":{"confirmation":null,"deviation":null,"unreviewed":false,"revoked":false},"status_confirmation":null,"status_deviation":null,"status_unreviewed":false,"status_confirmation_revoked":false}],"attachment":null}');
INSERT INTO report_recents VALUES(1,3);
PRAGMA user_version=7;
