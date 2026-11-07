// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Repository checks and generators for the Tallyline workspace.

#![forbid(unsafe_code)]

pub mod architecture;
pub mod build_code;
pub mod check;
pub mod dco;
pub mod deny;
pub mod dependabot;
pub mod english;
pub mod exit;
pub mod guard;
pub mod hashlist;
pub mod header;
pub mod install_cargo_mutants;
pub mod licence;
pub mod listing;
pub mod lockfile;
pub mod native;
pub mod native_assembly;
pub mod notices;
pub mod osv;
pub mod package;
pub mod property_scope;
pub mod property_scope_command;
pub mod quality;
pub mod register;
pub mod registry;
pub mod requirement;
pub mod sources;
pub mod sqlite_native;
pub mod tags;
pub mod target_graph;
pub mod tracked;
pub mod vendor;
pub mod vet;
pub mod work_paths;

#[cfg(test)]
mod denygate;
#[cfg(test)]
mod negative;
#[cfg(test)]
mod scratch;
#[cfg(test)]
mod token_reconciliation;

#[cfg(test)]
mod quality_tests;

#[cfg(test)]
mod native_scope_tests;

#[cfg(test)]
mod build_code_tests;

#[cfg(test)]
mod native_assembly_tests;

#[cfg(test)]
mod sqlite_native_tests;

#[cfg(test)]
mod build_code_workspace_tests;

mod ort_scope;

#[cfg(test)]
mod target_graph_tests;

#[cfg(test)]
mod mutation_tests;

pub mod mutation;
pub mod mutation_evidence;
mod mutation_uncoded;

#[cfg(test)]
mod mutation_evidence_tests;

#[cfg(test)]
mod package_archive_tests;

#[cfg(test)]
mod package_tests;

#[cfg(test)]
mod work_paths_tests;

#[cfg(test)]
mod check_paths_tests;

#[cfg(test)]
mod property_scope_tests;
pub mod reconstitution;

pub mod owned_process_scope;
