//! The rmcp server: tool surface and handler dispatch.
//!
//! Dispatch order is the Rung 0 contract:
//!
//! 1. `meister_lane_check` answers live — it IS the gate, exposed
//!    so the gate is demonstrable without storing anything.
//! 2. Every other catalog tool dispatches after the lane gate.
//!    For a record-carrying tool the lane gate (see `lane`) runs
//!    FIRST, before any handler logic: a refusal returns the
//!    typed lane error; an allowed envelope proceeds. Since
//!    Rung 1 the Group 1–2 tools are real implementations over
//!    the on-disk record store (see `store`); the remaining
//!    groups are typed seams naming their rung.
//! 3. An unknown name returns `unknown tool` as an `isError`
//!    result (execution failure, never a protocol error — Tiger
//!    Style MCP Rule 6).
//! 4. The profile check comes AFTER the lane gate. The server
//!    serves one profile per launch (`MEISTER_PROFILE`, see
//!    `catalog::PROFILES`): `tools/list` carries only that
//!    profile's tools, and a call to a catalog tool outside it
//!    returns the typed `group_not_loaded` error naming the
//!    profiles that carry the tool — never `unknown tool`. Because
//!    the lane gate runs first, refused input is never told where
//!    a tool lives. The refusal is pre-mutation: it returns before
//!    the store is resolved.
//!
//! The dispatch itself is stateless; the store root is resolved
//! per call from the environment contract (`store::ENV_ROOT`).
//! Modern era only: the era guard (see `guard`) owns the stdio
//! front door, and `supported_protocol_versions` pins this
//! handler to 2026-07-28.

use std::borrow::Cow;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    InitializeResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use serde_json::{Value, json};

use crate::catalog::{self, Profile, ToolSpec};
use crate::lane::{self, GateOutcome};
use crate::store::{Store, StoreError};

fn text_result(value: &Value, is_error: bool) -> CallToolResult {
    let content = vec![ContentBlock::text(value.to_string())];
    if is_error {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    }
}

fn err_result(msg: &str) -> CallToolResult {
    text_result(&json!({"error": msg}), true)
}

/// The seam answer: structured, typed, and honest about which
/// rung makes the tool real.
fn seam_result(spec: &ToolSpec) -> CallToolResult {
    text_result(
        &json!({"error": {
            "code": "not_implemented",
            "tool": spec.name,
            "group": spec.group,
            "rung": spec.rung,
            "message": format!(
                "{} is a typed seam at Rung 0; it becomes real at Rung {} \
                 (README §\"The path to 1.0\")",
                spec.name, spec.rung
            ),
        }}),
        true,
    )
}

/// A store outcome as a tool result: success carries the
/// operation's body; a failure carries the typed store code, the
/// reason, and — for an unverified census — the census itself,
/// so a caller sees which claims failed (D-713: never a bare
/// refusal where the evidence is the point).
fn store_result(outcome: Result<Value, StoreError>) -> CallToolResult {
    match outcome {
        Ok(body) => text_result(&body, false),
        Err(e) => {
            let mut error = json!({"code": e.code, "message": e.message});
            if let Some(census) = e.census {
                error["census"] = census;
            }
            text_result(&json!({"error": error}), true)
        }
    }
}

/// The lane refusal answer: typed code, the offending record id,
/// and the gate's reason. Produced before any mutation: the lane
/// gate runs before the store is touched, and the refusal path
/// writes nothing.
fn lane_refusal(code: &str, reason: &str, record_id: &str) -> CallToolResult {
    text_result(
        &json!({"error": {
            "code": code,
            "ids": [record_id],
            "message": reason,
        }}),
        true,
    )
}

/// The out-of-profile answer: the tool exists, but this launch
/// does not serve it. Names the profiles a client can relaunch
/// with. Produced before the store is resolved: it writes nothing.
fn group_not_loaded(spec: &ToolSpec, active: &Profile) -> CallToolResult {
    let carriers = catalog::carrying_profiles(spec);
    text_result(
        &json!({"error": {
            "code": "group_not_loaded",
            "tool": spec.name,
            "group": spec.group,
            "active_profile": active.name,
            "profiles": carriers,
            "message": format!(
                "{} is not loaded in profile {}; relaunch with {} set to one of: {}",
                spec.name,
                active.name,
                catalog::ENV_PROFILE,
                carriers.join(", ")
            ),
        }}),
        true,
    )
}

/// The dispatch. Pure over (profile, name, arguments):
/// unit-testable without any transport.
fn call_tool(profile: &Profile, name: &str, args: &Value) -> CallToolResult {
    if name == "meister_lane_check" {
        return text_result(&lane::verdict_json(args), false);
    }
    let Some(spec) = catalog::spec_named(name) else {
        return err_result("unknown tool");
    };
    if spec.record_carrying {
        let envelope = args.get("envelope").cloned().unwrap_or(Value::Null);
        if let GateOutcome::Refuse { code, reason } = lane::check_envelope(&envelope) {
            return lane_refusal(code, &reason, lane::record_id(&envelope));
        }
    }
    // The profile check follows the lane gate, by law: lane-refused
    // input never learns which profile carries a tool.
    if !profile.serves(&spec) {
        return group_not_loaded(&spec, profile);
    }
    // Rung 1 made Groups 1–2 real; Rung 2 adds system_define
    // (SYS-02..07), scene_define/scene_validate (SYS-08), and
    // level_compose's structural half (SYS-09; its narrative
    // fields still refuse until Rung 3). Every other unlisted
    // group stays a seam.
    let store = Store::from_env();
    match name {
        "meister_project_create" => store_result(store.create_project(args)),
        "meister_project_inspect" => store_result(store.inspect_project(args)),
        "meister_project_list" => store_result(store.list_projects(args)),
        "meister_cast_add" => store_result(store.add_cast(args)),
        "meister_cast_lock" => store_result(store.lock_cast(args)),
        "meister_canon_check" => store_result(store.canon_census(args)),
        "meister_system_define" => store_result(store.define_system(args)),
        "meister_scene_define" => store_result(store.define_scene(args)),
        "meister_scene_validate" => store_result(store.validate_scene(args)),
        "meister_level_compose" => store_result(store.compose_level(args)),
        "meister_dialogue_author" => store_result(store.author_dialogue(args)),
        "meister_dialogue_get" => store_result(store.get_dialogue(args)),
        "meister_reaction_set" => store_result(store.set_reaction(args)),
        "meister_story_define" => store_result(store.define_story(args)),
        "meister_playtest_generate" => store_result(store.generate_playtest(args)),
        "meister_verdict_record" => store_result(store.record_verdict(args)),
        _ => seam_result(&spec),
    }
}

/// The server handler. Dispatch is stateless; it runs on a
/// blocking thread so nothing in dispatch can stall the runtime.
/// The served profile is fixed at launch and never changes.
pub struct MeisterServer {
    profile: &'static Profile,
}

impl MeisterServer {
    pub fn new(profile: &'static Profile) -> Self {
        MeisterServer { profile }
    }
}

impl ServerHandler for MeisterServer {
    fn get_info(&self) -> InitializeResult {
        let mut server_info = Implementation::default();
        server_info.name = "meister".to_string();
        server_info.version = env!("CARGO_PKG_VERSION").to_string();
        let mut info = InitializeResult::default();
        info.protocol_version = ProtocolVersion::V_2026_07_28;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = server_info;
        info
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2026_07_28])
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: catalog::tool_definitions(self.profile),
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let name = request.name.to_string();
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let profile = self.profile;
        let outcome =
            tokio::task::spawn_blocking(move || call_tool(profile, &name, &arguments)).await;
        match outcome {
            Ok(result) => Ok(result.into()),
            Err(join) => Err(ErrorData::internal_error(
                format!("tool task failed: {join}"),
                None,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result_json(result: &CallToolResult) -> Value {
        let ContentBlock::Text(text) = &result.content[0] else {
            panic!("expected text content");
        };
        serde_json::from_str(&text.text).expect("result text is JSON")
    }

    fn profile(name: &str) -> &'static Profile {
        catalog::profile_named(name).expect("a declared profile")
    }

    /// The pre-profile dispatch tests run on the whole surface.
    fn full() -> &'static Profile {
        profile(catalog::PROFILE_FULL)
    }

    fn good_envelope() -> Value {
        json!({"record_id": "rec-9", "origin_lane": "qompass",
               "content_label": "sfw", "evidence": "README §Lane law"})
    }

    // ---------- validation ----------

    #[test]
    fn lane_check_allows_the_one_legal_combination() {
        let result = call_tool(full(), "meister_lane_check", &good_envelope());
        assert!(!result.is_error.unwrap_or(false));
        assert_eq!(result_json(&result)["verdict"], "allow");
    }

    #[test]
    fn allowed_envelope_reaches_the_seam() {
        // A Group 6 seam (Rung 6): the allowed envelope falls
        // through the gate to the seam error, unchanged.
        // (Groups 3-5 became real at Rungs 2-3.)
        let args = json!({"envelope": good_envelope(),
                          "project_id": "first-light"});
        let result = call_tool(full(), "meister_rating_record", &args);
        assert!(result.is_error.unwrap_or(false));
        let body = result_json(&result);
        assert_eq!(body["error"]["code"], "not_implemented");
        assert_eq!(body["error"]["rung"], 6);
    }

    #[test]
    fn in_profile_calls_dispatch_as_before() {
        // narrative carries Group 5: the call dispatches to
        // the real tool (playtest answers not_found against
        // the empty default store root — never the profile
        // refusal and never the seam).
        let args = json!({"envelope": good_envelope(),
                          "project_id": "first-light",
                          "graph_ref": "gate-arc"});
        for name in ["narrative", "niflheim-floor", "full"] {
            let result = call_tool(profile(name), "meister_playtest_generate", &args);
            assert_eq!(
                result_json(&result)["error"]["code"],
                "not_found",
                "profile {name}"
            );
        }
        // The lane check is core: live under every profile.
        for declared in catalog::PROFILES {
            let result = call_tool(declared, "meister_lane_check", &good_envelope());
            assert_eq!(
                result_json(&result)["verdict"],
                "allow",
                "{}",
                declared.name
            );
        }
    }

    // ---------- adversarial ----------

    #[test]
    fn out_of_profile_call_is_group_not_loaded_naming_the_carriers() {
        let args = json!({"envelope": good_envelope(),
                          "project_id": "first-light",
                          "scene_id": "floor-01",
                          "graph": {}});
        for name in ["core", "canon", "narrative", "assets", "qa", "release"] {
            let result = call_tool(profile(name), "meister_scene_define", &args);
            assert!(result.is_error.unwrap_or(false));
            let body = result_json(&result);
            assert_eq!(body["error"]["code"], "group_not_loaded", "profile {name}");
            assert_eq!(body["error"]["tool"], "meister_scene_define");
            assert_eq!(body["error"]["active_profile"], name);
            assert_eq!(
                body["error"]["profiles"],
                json!(["systems", "niflheim-floor"])
            );
        }
        // A read tool (no envelope, no gate) is refused the same way.
        let result = call_tool(
            profile("core"),
            "meister_account_inspect",
            &json!({"channel": "itch", "account_id": "a"}),
        );
        let body = result_json(&result);
        assert_eq!(body["error"]["code"], "group_not_loaded");
        assert_eq!(body["error"]["profiles"], json!(["release"]));
    }

    #[test]
    fn lane_gate_precedes_the_profile_check() {
        // Out of profile AND nsfw: the lane refusal wins, and the
        // answer says nothing about where the tool lives.
        let mut envelope = good_envelope();
        envelope["content_label"] = json!("nsfw");
        let args = json!({"envelope": envelope, "name": "x",
                          "engine": "bevy", "genre_pack": "puzzle"});
        let result = call_tool(profile("core"), "meister_project_create", &args);
        let body = result_json(&result);
        assert_eq!(body["error"]["code"], "lane_mismatch");
        assert!(body["error"].get("profiles").is_none());
        assert!(!body.to_string().contains("canon"));
        // Out of profile and unclassified: same order.
        let result = call_tool(
            profile("core"),
            "meister_project_create",
            &json!({"name": "x"}),
        );
        assert_eq!(result_json(&result)["error"]["code"], "lane_unclassified");
    }

    #[test]
    fn unknown_tool_stays_unknown_under_a_narrow_profile() {
        let result = call_tool(profile("core"), "meister_nope", &json!({}));
        assert_eq!(result_json(&result)["error"], "unknown tool");
    }

    #[test]
    fn nsfw_envelope_is_refused_before_the_seam() {
        let mut envelope = good_envelope();
        envelope["content_label"] = json!("nsfw");
        let args = json!({"envelope": envelope, "name": "x",
                          "engine": "bevy", "genre_pack": "puzzle"});
        let result = call_tool(full(), "meister_project_create", &args);
        let body = result_json(&result);
        assert_eq!(body["error"]["code"], "lane_mismatch");
        assert_eq!(body["error"]["ids"], json!(["rec-9"]));
    }

    #[test]
    fn missing_envelope_is_unclassified_not_a_crash() {
        let result = call_tool(full(), "meister_project_create", &json!({"name": "x"}));
        let body = result_json(&result);
        assert_eq!(body["error"]["code"], "lane_unclassified");
    }

    #[test]
    fn unknown_tool_is_an_error_result() {
        let result = call_tool(full(), "meister_nope", &json!({}));
        assert!(result.is_error.unwrap_or(false));
        assert_eq!(result_json(&result)["error"], "unknown tool");
    }
}
