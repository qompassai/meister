//! The rmcp server: tool surface and handler dispatch.
//!
//! Dispatch order is the Rung 0 contract:
//!
//! 1. `meister_lane_check` answers live — it IS the gate, exposed
//!    so the gate is demonstrable without storing anything.
//! 2. Every other catalog tool is a typed seam. For a
//!    record-carrying tool the lane gate (see `lane`) runs FIRST,
//!    before any handler logic: a refusal returns the typed lane
//!    error; an allowed envelope falls through to the seam error,
//!    which names the rung that makes the tool real.
//! 3. An unknown name returns `unknown tool` as an `isError`
//!    result (execution failure, never a protocol error — Tiger
//!    Style MCP Rule 6).
//!
//! The server is stateless and holds no store at Rung 0: nothing
//! is written anywhere, by any call, allowed or refused. Modern
//! era only: the era guard (see `guard`) owns the stdio front
//! door, and `supported_protocol_versions` pins this handler to
//! 2026-07-28.

use std::borrow::Cow;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    InitializeResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities,
    Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use serde_json::{Value, json};

use crate::catalog::{self, ToolSpec};
use crate::lane::{self, GateOutcome};

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

/// The lane refusal answer: typed code, the offending record id,
/// and the gate's reason. Produced before any mutation — at Rung 0
/// there is no state to mutate, and the refusal path writes none.
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

/// The dispatch. Pure over (name, arguments): unit-testable
/// without any transport.
fn call_tool(name: &str, args: &Value) -> CallToolResult {
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
    seam_result(&spec)
}

/// The server handler. Dispatch is stateless; it runs on a
/// blocking thread so nothing in dispatch can stall the runtime.
pub struct MeisterServer;

impl MeisterServer {
    pub fn new() -> Self {
        MeisterServer
    }
}

impl Default for MeisterServer {
    fn default() -> Self {
        Self::new()
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
            tools: tool_definitions(),
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
        let outcome = tokio::task::spawn_blocking(move || call_tool(&name, &arguments)).await;
        match outcome {
            Ok(result) => Ok(result.into()),
            Err(join) => Err(ErrorData::internal_error(
                format!("tool task failed: {join}"),
                None,
            )),
        }
    }
}

/// The served definitions (kept as a free function so the catalog
/// tests and the handler share one construction path).
fn tool_definitions() -> Vec<Tool> {
    catalog::tool_definitions()
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

    fn good_envelope() -> Value {
        json!({"record_id": "rec-9", "origin_lane": "qompass",
               "content_label": "sfw", "evidence": "README §Lane law"})
    }

    // ---------- validation ----------

    #[test]
    fn lane_check_allows_the_one_legal_combination() {
        let result = call_tool("meister_lane_check", &good_envelope());
        assert!(!result.is_error.unwrap_or(false));
        assert_eq!(result_json(&result)["verdict"], "allow");
    }

    #[test]
    fn allowed_envelope_reaches_the_seam() {
        let args = json!({"envelope": good_envelope(), "name": "First Light",
                          "engine": "bevy", "genre_pack": "puzzle"});
        let result = call_tool("meister_project_create", &args);
        assert!(result.is_error.unwrap_or(false));
        let body = result_json(&result);
        assert_eq!(body["error"]["code"], "not_implemented");
        assert_eq!(body["error"]["rung"], 1);
    }

    // ---------- adversarial ----------

    #[test]
    fn nsfw_envelope_is_refused_before_the_seam() {
        let mut envelope = good_envelope();
        envelope["content_label"] = json!("nsfw");
        let args = json!({"envelope": envelope, "name": "x",
                          "engine": "bevy", "genre_pack": "puzzle"});
        let result = call_tool("meister_project_create", &args);
        let body = result_json(&result);
        assert_eq!(body["error"]["code"], "lane_mismatch");
        assert_eq!(body["error"]["ids"], json!(["rec-9"]));
    }

    #[test]
    fn missing_envelope_is_unclassified_not_a_crash() {
        let result = call_tool("meister_project_create", &json!({"name": "x"}));
        let body = result_json(&result);
        assert_eq!(body["error"]["code"], "lane_unclassified");
    }

    #[test]
    fn unknown_tool_is_an_error_result() {
        let result = call_tool("meister_nope", &json!({}));
        assert!(result.is_error.unwrap_or(false));
        assert_eq!(result_json(&result)["error"], "unknown tool");
    }
}
