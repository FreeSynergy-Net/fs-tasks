//! Entwurfsort für Trait-Deltas, die noch nicht gesperrt sind (Workflow trait-delta). Heute: `tasks_v2`, das Delta D1
//! zu `tasks_v1` (Board: Stufe, Spalten, Notiz, Live-Feed; ADR 0049 Nachtrag 2026-11-17), mit seiner Probe.
//!
//! Pattern: Repository + Port (der Trait liegt in `tasks_v2/`; die Konformitätsfälle K1–K30 in `conformance_v2`).
//!
//! Fester Ort nach dem Fixpunkt: das Träger-Crate von fs-tasks (Regel 8); `tasks_v1/` bleibt unberührt. Die Schema-Kopie
//! `tasks_v2.capnp` ist `tasks.capnp` aus fs-interfaces v0.0.27 plus die angehängten Zeilen (nur Einfügungen).

// Wie `tasks_v1` (Quelle: das 2021er Probe-Crate): nicht umformatiert, damit die byte-gleichen Dateien byte-gleich
// bleiben (`conformance_v2::pure` prüft, was es hier prüfen kann), und mit denselben Ausnahmen.
#[rustfmt::skip]
#[allow(
    missing_docs,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::module_name_repetitions,
    clippy::large_enum_variant,
    clippy::wildcard_imports,
    clippy::too_many_lines
)]
pub mod tasks_v2;

#[cfg(test)]
mod conformance_v2;
