//! TEACH-01 — the teaching layer v1: the server-side validators
//! of the Educational pack's two kinds (`packs/educational/`).
//!
//! The pack's field tables (PK-01) check presence and JSON types;
//! these validators add the teaching law on top, after the field
//! table has passed:
//!
//! - `curriculum` (a canon kind): a domain, a codex principle and
//!   a failure class, each non-empty text.
//! - `assessment_encounter`: names its gate suite by path plus
//!   SHA-256 and records the suite's validation/adversarial split.
//!   **Meister never runs player code; the game does** — the
//!   suite is a pinned reference and a recorded split, never
//!   something executed here.
//! - Every encounter declares one embedding pattern, and a quiz
//!   frame (an encounter carrying a `quiz`) is
//!   `narrative_quiz_frame` and only that.
//!
//! Validation is pure: no store, no files. Every refusal is typed:
//! `split_unbalanced`, `suite_unhashed`, `embedding_mismatch`, and
//! `record_invalid` for any other malformed value.
//!
//! Reached through `pack::Selection::validate`, which has no live
//! call path until a carrier tool is wired (an RPG-01 decision,
//! per PK-01); the same `expect` guard applies here, so wiring the
//! carrier makes this expectation fail until it is removed.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "reached via pack::Selection, which RPG-01 wires to a carrier"
    )
)]

use serde_json::Value;

use crate::pack::{PackError, PackResult};

/// The four ways a lesson sits in play (PATH-TO-1.0, TEACH-01).
pub const EMBEDDING_PATTERNS: &[&str] = &[
    "lesson_as_mechanic",
    "lesson_as_gate",
    "assessment_as_encounter",
    "narrative_quiz_frame",
];

/// The gate suite reference's keys; the object is closed.
const SUITE_KEYS: &[&str] = &["path", "sha256", "validation_count", "adversarial_count"];

const MAX_ID_CHARS: usize = 64;
const MAX_TEXT_CHARS: usize = 500;
const MAX_QUIZ_ITEMS: usize = 64;
/// A recorded suite size beyond this is a recording error, not a
/// suite.
const MAX_SUITE_COUNT: u64 = 100_000;

fn refuse(code: &'static str, message: impl Into<String>) -> PackError {
    PackError {
        code,
        message: message.into(),
    }
}

fn invalid(message: impl Into<String>) -> PackError {
    refuse("record_invalid", message)
}

/// The teaching law for one Educational-pack record whose field
/// table already passed. Kinds this layer does not own pass
/// through untouched.
pub fn validate(kind: &str, record: &Value) -> PackResult<()> {
    match kind {
        "curriculum" => validate_curriculum(record),
        "assessment_encounter" => validate_encounter(record),
        _ => Ok(()),
    }
}

/// A present string field, non-empty and within `max` chars.
fn text<'v>(record: &'v Value, field: &str, max: usize) -> PackResult<&'v str> {
    let value = record[field]
        .as_str()
        .ok_or_else(|| invalid(format!("{field} must be text")))?;
    if value.trim().is_empty() || value.chars().count() > max {
        return Err(invalid(format!(
            "{field} must be non-empty text of at most {max} characters"
        )));
    }
    Ok(value)
}

fn validate_curriculum(record: &Value) -> PackResult<()> {
    text(record, "curriculum_id", MAX_ID_CHARS)?;
    for field in ["domain", "codex_principle", "failure_class"] {
        text(record, field, MAX_TEXT_CHARS)?;
    }
    Ok(())
}

fn validate_encounter(record: &Value) -> PackResult<()> {
    let id = text(record, "encounter_id", MAX_ID_CHARS)?;
    text(record, "curriculum_id", MAX_ID_CHARS)?;
    let pattern = text(record, "embedding_pattern", MAX_ID_CHARS)?;
    if !EMBEDDING_PATTERNS.contains(&pattern) {
        return Err(invalid(format!(
            "encounter {id} names embedding pattern {pattern}; the patterns are {}",
            EMBEDDING_PATTERNS.join(", ")
        )));
    }
    validate_suite(id, &record["gate_suite"])?;
    match record.get("quiz") {
        Some(quiz) => {
            let items = quiz
                .as_array()
                .ok_or_else(|| invalid(format!("encounter {id} quiz must be an array")))?;
            if items.is_empty() || items.len() > MAX_QUIZ_ITEMS {
                return Err(invalid(format!(
                    "encounter {id} quiz holds 1..={MAX_QUIZ_ITEMS} items"
                )));
            }
            if pattern != "narrative_quiz_frame" {
                return Err(refuse(
                    "embedding_mismatch",
                    format!(
                        "encounter {id} is a quiz frame but claims {pattern}; a quiz frame is narrative_quiz_frame"
                    ),
                ));
            }
        }
        None if pattern == "narrative_quiz_frame" => {
            return Err(refuse(
                "embedding_mismatch",
                format!("encounter {id} claims narrative_quiz_frame but carries no quiz"),
            ));
        }
        None => {}
    }
    Ok(())
}

/// The gate suite: a path and its SHA-256 pin, plus the recorded
/// split. Meister checks the record; it never runs the suite.
fn validate_suite(id: &str, suite: &Value) -> PackResult<()> {
    let obj = suite
        .as_object()
        .ok_or_else(|| invalid(format!("encounter {id} gate_suite must be an object")))?;
    if let Some(extra) = obj.keys().find(|k| !SUITE_KEYS.contains(&k.as_str())) {
        return Err(invalid(format!(
            "encounter {id} gate_suite declares no field {extra}"
        )));
    }
    text(suite, "path", MAX_TEXT_CHARS)?;
    match obj.get("sha256") {
        None | Some(Value::Null) => {
            return Err(refuse(
                "suite_unhashed",
                format!("encounter {id} names a gate suite without its SHA-256"),
            ));
        }
        Some(Value::String(hash))
            if hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) => {}
        Some(_) => {
            return Err(invalid(format!(
                "encounter {id} gate_suite sha256 must be 64 lowercase hex characters"
            )));
        }
    }
    let mut counts = [0u64; 2];
    for (slot, field) in ["validation_count", "adversarial_count"].iter().enumerate() {
        counts[slot] = obj
            .get(*field)
            .and_then(Value::as_u64)
            .filter(|n| *n <= MAX_SUITE_COUNT)
            .ok_or_else(|| {
                invalid(format!(
                    "encounter {id} gate_suite {field} must be an integer 0..={MAX_SUITE_COUNT}"
                ))
            })?;
    }
    let [validation, adversarial] = counts;
    if validation == 0 && adversarial == 0 {
        return Err(invalid(format!(
            "encounter {id} gate_suite records no tests"
        )));
    }
    if validation != adversarial {
        return Err(refuse(
            "split_unbalanced",
            format!(
                "encounter {id} gate_suite records {validation} validation and {adversarial} adversarial tests; the split is half and half"
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::{Registry, check_compiled};
    use serde_json::json;

    const EDUCATIONAL: &str = include_str!("../packs/educational/pack.json");

    fn registry() -> Registry {
        Registry::from_sources(&[("educational", EDUCATIONAL)]).expect("educational pack validates")
    }

    pub(super) fn curriculum() -> Value {
        json!({"curriculum_id": "ownership", "domain": "rust",
               "codex_principle": "every value has exactly one owner",
               "failure_class": "use after move"})
    }

    pub(super) fn encounter(pattern: &str) -> Value {
        let mut rec = json!({"encounter_id": "foe-borrowck", "curriculum_id": "ownership",
            "embedding_pattern": pattern,
            "gate_suite": {"path": "suites/floor-12/borrowck.rs",
                           "sha256": "54bda4c4bc0a077e32d165cc5253b924c855b3bf1010d376be7d48b4847f39bd",
                           "validation_count": 6, "adversarial_count": 6}});
        if pattern == "narrative_quiz_frame" {
            rec["quiz"] = json!(["who owns the value after the move?"]);
        }
        rec
    }

    #[test]
    fn both_kinds_are_in_the_compiled_educational_pack() {
        assert_eq!(check_compiled(), Ok(1));
        let doc: Value = serde_json::from_str(EDUCATIONAL).unwrap();
        let kinds: Vec<&str> = doc["kinds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["curriculum", "assessment_encounter"]);
        assert_eq!(doc["slug"], "educational");
    }

    #[test]
    fn a_curriculum_and_encounter_pair_validates_as_a_secondary_pack() {
        let reg = registry();
        let sel = reg.select("role_playing", &["educational"]).unwrap();
        assert_eq!(sel.validate("curriculum", &curriculum()), Ok(()));
        assert_eq!(
            sel.validate(
                "assessment_encounter",
                &encounter("assessment_as_encounter")
            ),
            Ok(())
        );
    }

    #[test]
    fn every_embedding_pattern_validates_in_its_honest_form() {
        let reg = registry();
        let sel = reg.select("role_playing", &["educational"]).unwrap();
        for pattern in EMBEDDING_PATTERNS {
            assert_eq!(
                sel.validate("assessment_encounter", &encounter(pattern)),
                Ok(()),
                "{pattern}"
            );
        }
    }

    #[test]
    fn the_progress_state_spec_is_inspectable_data() {
        // The progress-state spec a teacher or learner inspects is
        // the pack's learner_progress template: plain states and
        // transitions, from unseen to mastered, with a failed
        // assessment returning to practice.
        let doc: Value = serde_json::from_str(EDUCATIONAL).unwrap();
        let template = &doc["systems_templates"][0];
        assert_eq!(template["template_id"], "learner_progress");
        assert_eq!(template["system_kind"], "state_machine");
        let spec = &template["spec"];
        assert_eq!(spec["initial"], "unseen");
        assert_eq!(spec["terminal"], json!(["mastered"]));
        assert!(
            spec["transitions"]
                .as_array()
                .unwrap()
                .contains(&json!({"from": "assessed", "to": "practiced"}))
        );
    }

    fn refused(kind: &str, record: &Value) -> &'static str {
        let reg = registry();
        let sel = reg.select("role_playing", &["educational"]).unwrap();
        sel.validate(kind, record).unwrap_err().code
    }

    #[test]
    fn adv_an_unequal_recorded_split_is_split_unbalanced() {
        let mut rec = encounter("lesson_as_gate");
        rec["gate_suite"]["adversarial_count"] = json!(5);
        assert_eq!(refused("assessment_encounter", &rec), "split_unbalanced");
        rec["gate_suite"]["validation_count"] = json!(0);
        rec["gate_suite"]["adversarial_count"] = json!(3);
        assert_eq!(refused("assessment_encounter", &rec), "split_unbalanced");
    }

    #[test]
    fn adv_a_suite_without_a_hash_is_refused() {
        let mut rec = encounter("lesson_as_gate");
        rec["gate_suite"].as_object_mut().unwrap().remove("sha256");
        assert_eq!(refused("assessment_encounter", &rec), "suite_unhashed");
        rec["gate_suite"]["sha256"] = Value::Null;
        assert_eq!(refused("assessment_encounter", &rec), "suite_unhashed");
        rec["gate_suite"]["sha256"] = json!("not-a-hash");
        assert_eq!(refused("assessment_encounter", &rec), "record_invalid");
    }

    #[test]
    fn adv_a_quiz_frame_claiming_lesson_as_mechanic_is_refused() {
        let mut rec = encounter("lesson_as_mechanic");
        rec["quiz"] = json!(["what does the borrow checker refuse?"]);
        assert_eq!(refused("assessment_encounter", &rec), "embedding_mismatch");
        let mut bare = encounter("narrative_quiz_frame");
        bare.as_object_mut().unwrap().remove("quiz");
        assert_eq!(refused("assessment_encounter", &bare), "embedding_mismatch");
    }

    #[test]
    fn adv_a_curriculum_without_its_codex_principle_is_refused() {
        // The pack's field table: every curriculum names domain,
        // codex principle and failure class.
        let mut cur = curriculum();
        cur.as_object_mut().unwrap().remove("codex_principle");
        assert_eq!(refused("curriculum", &cur), "record_invalid");
    }

    #[test]
    fn adv_teaching_records_outside_the_law_are_refused() {
        // Unknown pattern, a smuggled suite key, empty curriculum
        // text, and the kinds unselected when Educational is not a
        // selected pack.
        let mut rec = encounter("lesson_as_gate");
        rec["embedding_pattern"] = json!("lesson_as_vibe");
        assert_eq!(refused("assessment_encounter", &rec), "record_invalid");
        let mut rec = encounter("lesson_as_gate");
        rec["gate_suite"]["run"] = json!("cargo test");
        assert_eq!(refused("assessment_encounter", &rec), "record_invalid");
        let mut cur = curriculum();
        cur["failure_class"] = json!("  ");
        assert_eq!(refused("curriculum", &cur), "record_invalid");
        let reg = registry();
        let rpg_only = reg.select("role_playing", &[]).unwrap();
        assert_eq!(
            rpg_only
                .validate("curriculum", &curriculum())
                .unwrap_err()
                .code,
            "pack_kind_not_selected"
        );
    }
}
