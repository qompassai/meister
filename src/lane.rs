//! The lane gate: Meister's one live enforcement at Rung 0.
//!
//! Lane law (README §"Lane law (never softened)"): every record
//! Meister creates carries `origin_lane: qompass` and
//! `content_label: sfw` from birth, with evidence. Intake accepts
//! only that combination. This module is the shared guard every
//! record-carrying tool runs BEFORE any handler logic:
//!
//! - `content_label: nsfw` (any origin), a non-sfw label arriving
//!   from the other lane, or an sfw label claiming the other lane's
//!   origin → typed refusal `lane_mismatch`.
//! - A missing or unreadable label, a missing origin on an sfw
//!   record, any other unrecognised combination, or missing
//!   evidence → typed refusal `lane_unclassified`. Unclassified
//!   defaults to the NSFW side: fail closed.
//! - Only `origin_lane: qompass` + `content_label: sfw` + non-empty
//!   `evidence` proceeds.
//!
//! Refusals name the offending record id and happen before any
//! mutation: at Rung 0 the server holds no state and writes
//! nothing, and a refused call must leave that exactly as true as
//! an allowed one.

use serde_json::{Value, json};

/// The only origin lane whose records Meister accepts.
pub const ORIGIN_QOMPASS: &str = "qompass";
/// The other lane's origin stamp; its records never cross.
pub const ORIGIN_OTHER_LANE: &str = "ntranch";
/// The only content label Meister accepts.
pub const LABEL_SFW: &str = "sfw";
/// The label that is always a mismatch here.
pub const LABEL_NSFW: &str = "nsfw";

/// Refusal code: the record belongs to the other lane.
pub const CODE_MISMATCH: &str = "lane_mismatch";
/// Refusal code: the record's classification cannot be established.
pub const CODE_UNCLASSIFIED: &str = "lane_unclassified";

/// The gate's decision on one candidate envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
    /// The envelope is a born-classified Qompass SFW record.
    Allow,
    /// Refused before any handler logic or mutation.
    Refuse {
        /// Stable typed code (`lane_mismatch` / `lane_unclassified`).
        code: &'static str,
        /// Human-readable reason, safe to return to the caller.
        reason: String,
    },
}

impl GateOutcome {
    fn refuse(code: &'static str, reason: impl Into<String>) -> Self {
        GateOutcome::Refuse {
            code,
            reason: reason.into(),
        }
    }
}

/// Extract a non-empty string field from an envelope object.
fn field_str<'a>(envelope: &'a Value, field: &str) -> Option<&'a str> {
    envelope
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Decide one envelope. `envelope` is the raw `envelope` argument
/// object of a record-carrying tool call (or the whole argument of
/// `meister_lane_check`); anything that is not a JSON object is
/// unclassified by shape.
pub fn check_envelope(envelope: &Value) -> GateOutcome {
    if !envelope.is_object() {
        return GateOutcome::refuse(CODE_UNCLASSIFIED, "envelope is not a JSON object");
    }
    let origin = field_str(envelope, "origin_lane");
    let label = field_str(envelope, "content_label");
    let evidence = field_str(envelope, "evidence");
    check_fields(origin, label, evidence)
}

/// Decide from the three classification fields directly. This is
/// the whole law; the match order matters — an nsfw label is a
/// mismatch no matter what else the envelope claims.
pub fn check_fields(
    origin: Option<&str>,
    label: Option<&str>,
    evidence: Option<&str>,
) -> GateOutcome {
    let Some(label) = label else {
        return GateOutcome::refuse(CODE_UNCLASSIFIED, "content_label is missing");
    };
    if label == LABEL_NSFW {
        return GateOutcome::refuse(
            CODE_MISMATCH,
            "content_label nsfw is refused in the Qompass lane",
        );
    }
    if label != LABEL_SFW {
        // A label Meister does not recognise: from the other lane
        // it is a mismatch; from anywhere else it is unclassified.
        return if origin == Some(ORIGIN_OTHER_LANE) {
            GateOutcome::refuse(
                CODE_MISMATCH,
                format!("content_label {label} arrived from origin_lane {ORIGIN_OTHER_LANE}"),
            )
        } else {
            GateOutcome::refuse(
                CODE_UNCLASSIFIED,
                format!("content_label {label} is not a recognised classification"),
            )
        };
    }
    // label == sfw: the origin decides.
    match origin {
        Some(ORIGIN_QOMPASS) => {
            if evidence.is_none() {
                return GateOutcome::refuse(
                    CODE_UNCLASSIFIED,
                    "evidence is missing; a claimed label needs its evidence",
                );
            }
            GateOutcome::Allow
        }
        Some(ORIGIN_OTHER_LANE) => GateOutcome::refuse(
            CODE_MISMATCH,
            "origin_lane ntranch records never cross into the Qompass lane",
        ),
        _ => GateOutcome::refuse(
            CODE_UNCLASSIFIED,
            "origin_lane is missing or unrecognised on an sfw record",
        ),
    }
}

/// The record id a refusal names: the envelope's `record_id`, or
/// the empty string when the envelope does not carry one.
pub fn record_id(envelope: &Value) -> &str {
    field_str(envelope, "record_id").unwrap_or("")
}

/// The `meister_lane_check` verdict payload for one candidate
/// envelope. Pure: stores nothing, mutates nothing.
pub fn verdict_json(envelope: &Value) -> Value {
    match check_envelope(envelope) {
        GateOutcome::Allow => json!({
            "verdict": "allow",
            "refusal_code": Value::Null,
            "reason": "origin_lane qompass + content_label sfw + evidence present",
        }),
        GateOutcome::Refuse { code, reason } => json!({
            "verdict": "refuse",
            "refusal_code": code,
            "reason": reason,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(origin: &str, label: &str, evidence: &str) -> Value {
        json!({"record_id": "rec-1", "origin_lane": origin,
               "content_label": label, "evidence": evidence})
    }

    // ---------- validation ----------

    #[test]
    fn sfw_qompass_with_evidence_allows() {
        assert_eq!(
            check_envelope(&env("qompass", "sfw", "README §Lane law")),
            GateOutcome::Allow
        );
        let verdict = verdict_json(&env("qompass", "sfw", "gate record 1"));
        assert_eq!(verdict["verdict"], "allow");
        assert_eq!(verdict["refusal_code"], Value::Null);
    }

    // ---------- adversarial ----------

    #[test]
    fn nsfw_label_is_mismatch_from_any_origin() {
        for origin in ["qompass", "ntranch", "elsewhere"] {
            match check_envelope(&env(origin, "nsfw", "x")) {
                GateOutcome::Refuse { code, .. } => assert_eq!(code, CODE_MISMATCH),
                other => panic!("expected mismatch for {origin}, got {other:?}"),
            }
        }
    }

    #[test]
    fn other_lane_origin_is_mismatch_even_when_labelled_sfw() {
        match check_envelope(&env("ntranch", "sfw", "x")) {
            GateOutcome::Refuse { code, .. } => assert_eq!(code, CODE_MISMATCH),
            other => panic!("expected mismatch, got {other:?}"),
        }
    }

    #[test]
    fn missing_label_is_unclassified() {
        let bare = json!({"record_id": "r", "origin_lane": "qompass", "evidence": "x"});
        match check_envelope(&bare) {
            GateOutcome::Refuse { code, .. } => assert_eq!(code, CODE_UNCLASSIFIED),
            other => panic!("expected unclassified, got {other:?}"),
        }
    }

    #[test]
    fn missing_evidence_is_unclassified() {
        match check_envelope(&env("qompass", "sfw", "")) {
            GateOutcome::Refuse { code, .. } => assert_eq!(code, CODE_UNCLASSIFIED),
            other => panic!("expected unclassified, got {other:?}"),
        }
    }

    #[test]
    fn unknown_label_and_non_object_are_unclassified() {
        match check_envelope(&env("qompass", "unrated", "x")) {
            GateOutcome::Refuse { code, .. } => assert_eq!(code, CODE_UNCLASSIFIED),
            other => panic!("expected unclassified, got {other:?}"),
        }
        match check_envelope(&json!("not an object")) {
            GateOutcome::Refuse { code, .. } => assert_eq!(code, CODE_UNCLASSIFIED),
            other => panic!("expected unclassified, got {other:?}"),
        }
        match check_envelope(&json!({"content_label": 42})) {
            GateOutcome::Refuse { code, .. } => assert_eq!(code, CODE_UNCLASSIFIED),
            other => panic!("expected unclassified, got {other:?}"),
        }
    }

    #[test]
    fn unknown_label_from_other_lane_is_mismatch() {
        match check_envelope(&env("ntranch", "adult", "x")) {
            GateOutcome::Refuse { code, .. } => assert_eq!(code, CODE_MISMATCH),
            other => panic!("expected mismatch, got {other:?}"),
        }
    }
}
