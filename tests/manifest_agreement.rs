//! Manifest agreement: the agent card and server.json are a
//! provisioning pair (D-1605). Client configs for the estate's
//! agent runtimes are to be generated from card + server.json
//! pairs, so the join keys must agree exactly, from birth:
//!
//! - card `name` == server.json `title`
//! - card `version` == server.json `version`
//! - the card's MCP-interface extension `registryName` ==
//!   server.json `name`
//!
//! The card must also stay honest AND spec-valid (D-1607): it
//! declares exactly one interface, bound with the qompass MCP
//! custom binding (a URI protocolBinding, per the A2A v1.0
//! custom-transport rule) whose url carries the stdio command
//! under that binding's own semantics; no core A2A binding is
//! claimed, and the invocation facts ride in the extension
//! params.
//!
//! Since EFLO-04 the card also lists one invocation entry per
//! surface profile under that extension, so a client config can
//! be generated per profile from the card alone. The entries must
//! equal the server's declared profiles (`PROFILES` in
//! `src/catalog.rs`) — same names, same order, none missing, none
//! extra, none twice. The declared list is read from the built
//! binary itself (its typed refusal names every declared profile),
//! and every card entry is launched to prove it selects a real
//! profile. server.json stays single-identity: it names no
//! profile.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn read_json(rel: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

#[test]
fn card_and_server_json_agree_on_the_join_keys() {
    let card = read_json(".well-known/agent-card.json");
    let server = read_json("server.json");
    assert_eq!(card["name"], server["title"], "card name / server title");
    assert_eq!(
        card["version"], server["version"],
        "card version / server version"
    );
    let extension = &card["capabilities"]["extensions"][0];
    assert_eq!(
        extension["params"]["registryName"], server["name"],
        "extension registryName / server name"
    );
}

#[test]
fn card_declares_no_a2a_endpoint_and_carries_invocation() {
    let card = read_json(".well-known/agent-card.json");
    // Spec-valid honesty (D-1607): supportedInterfaces is a
    // required, non-empty field in A2A v1.0, so the card
    // declares exactly one interface under the qompass MCP
    // custom binding (URI protocolBinding); its url carries
    // the stdio command under that binding's semantics. No
    // core A2A binding (JSONRPC / GRPC / HTTP+JSON) is claimed.
    let interfaces = card["supportedInterfaces"].as_array().expect("interfaces");
    assert_eq!(interfaces.len(), 1, "exactly one declared interface");
    assert_eq!(
        interfaces[0]["protocolBinding"],
        "https://qompass.ai/extensions/mcp-interface/v1"
    );
    assert_eq!(interfaces[0]["url"], "~/.local/bin/meister");
    assert_eq!(interfaces[0]["protocolVersion"], "1.0");
    // A field removed in A2A v1.0 must never reappear.
    assert!(
        card["capabilities"].get("stateTransitionHistory").is_none(),
        "stateTransitionHistory was removed in A2A v1.0"
    );
    let invocation = &card["capabilities"]["extensions"][0]["params"]["invocation"];
    assert_eq!(invocation["transport"], "stdio");
    assert_eq!(invocation["command"], "~/.local/bin/meister");
    assert_eq!(invocation["args"], serde_json::json!([]));
    assert_eq!(invocation["protocolEra"], "2026-07-28");
    // The ten tool groups are the card's skills.
    assert_eq!(card["skills"].as_array().map(Vec::len), Some(10));
}

// ---------- EFLO-04: one invocation entry per profile ----------

const PROFILE_ENV: &str = "MEISTER_PROFILE";

/// Launch the built binary with exactly `env` (the card entry's
/// environment) and ask for `tools/list`.
fn launch(env: &serde_json::Map<String, serde_json::Value>) -> Output {
    let store = std::env::temp_dir().join(format!("meister-manifest-store-{}", std::process::id()));
    let mut command = Command::new(env!("CARGO_BIN_EXE_meister"));
    command
        .current_dir(std::env::temp_dir())
        .env("MEISTER_PROJECTS_ROOT", store)
        .env_remove(PROFILE_ENV)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value.as_str().expect("env values are strings"));
    }
    let mut child = command.spawn().unwrap();
    {
        let meta = serde_json::json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {"name": "meister-manifest-test", "version": "0"},
            "io.modelcontextprotocol/clientCapabilities": {}});
        let stdin = child.stdin.as_mut().unwrap();
        for (id, method) in [(0, "server/discover"), (1, "tools/list")] {
            let frame = serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method,
                                           "params": {"_meta": meta}});
            // A server that refused to start has closed the pipe.
            let _ = writeln!(stdin, "{frame}");
        }
    }
    child.wait_with_output().unwrap()
}

/// The tool names a successful launch served.
fn served(output: &Output) -> BTreeSet<String> {
    assert!(
        output.status.success(),
        "launch failed: {:?}",
        output.status
    );
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    let list = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|frame| frame["id"] == 1)
        .expect("a tools/list answer");
    list["result"]["tools"]
        .as_array()
        .expect("a tools array")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect()
}

/// The server's declared profiles, in declared order, from the
/// binary itself: its unknown-profile refusal names them all.
fn declared_profiles() -> Vec<String> {
    let mut env = serde_json::Map::new();
    env.insert(PROFILE_ENV.into(), "not-a-declared-profile".into());
    let output = launch(&env);
    assert_eq!(output.status.code(), Some(2), "the probe must be refused");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let list = stderr
        .split_once("(declared: ")
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(list, _)| list)
        .unwrap_or_else(|| panic!("no declared list in: {stderr}"));
    list.split(", ").map(str::to_string).collect()
}

/// The card's profile law. Returns the first violation.
fn check_card_profiles(card: &serde_json::Value, declared: &[String]) -> Result<(), String> {
    let params = &card["capabilities"]["extensions"][0]["params"];
    let base = &params["invocation"];
    let entries = params["profiles"]
        .as_array()
        .ok_or("the extension params carry no profiles array")?;
    let mut names: Vec<&str> = Vec::new();
    for entry in entries {
        let name = entry["profile"]
            .as_str()
            .ok_or("a profile entry has no string name")?;
        if names.contains(&name) {
            return Err(format!("duplicate profile {name}"));
        }
        names.push(name);
        if !declared.iter().any(|d| d == name) {
            return Err(format!("profile {name} is not declared by the server"));
        }
        if !entry["description"].as_str().is_some_and(|d| !d.is_empty()) {
            return Err(format!("profile {name} has no description"));
        }
        // The entry is a whole invocation: the base invocation
        // plus exactly the one environment variable.
        let invocation = &entry["invocation"];
        for key in ["transport", "command", "args", "protocolEra"] {
            if invocation[key].is_null() || invocation[key] != base[key] {
                return Err(format!(
                    "profile {name}: invocation {key} differs from the base"
                ));
            }
        }
        if invocation["env"] != serde_json::json!({PROFILE_ENV: name}) {
            return Err(format!("profile {name}: env does not select the profile"));
        }
    }
    for name in declared {
        if !names.contains(&name.as_str()) {
            return Err(format!("declared profile {name} is missing from the card"));
        }
    }
    if names != declared.iter().map(String::as_str).collect::<Vec<_>>() {
        return Err("card profiles are not in declared order".into());
    }
    let default = params["defaultProfile"]
        .as_str()
        .ok_or("the extension params name no defaultProfile")?;
    if !names.contains(&default) {
        return Err(format!("defaultProfile {default} is not a listed profile"));
    }
    Ok(())
}

fn profile_entries(card: &mut serde_json::Value) -> &mut Vec<serde_json::Value> {
    card["capabilities"]["extensions"][0]["params"]["profiles"]
        .as_array_mut()
        .expect("profiles array")
}

// ---------- validation ----------

#[test]
fn card_profiles_equal_the_declared_profiles() {
    let card = read_json(".well-known/agent-card.json");
    let declared = declared_profiles();
    assert_eq!(declared.len(), 9, "declared: {declared:?}");
    assert_eq!(check_card_profiles(&card, &declared), Ok(()));
    // server.json stays single-identity: no profile appears in it.
    let server =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("server.json"))
            .unwrap();
    assert!(!server.contains(PROFILE_ENV) && !server.contains("profiles"));
    // And the card still declares exactly one interface (D-1607).
    assert_eq!(
        card["supportedInterfaces"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn every_card_profile_launches_from_the_card_alone() {
    let card = read_json(".well-known/agent-card.json");
    let params = &card["capabilities"]["extensions"][0]["params"];
    let mut sets = std::collections::BTreeMap::new();
    for entry in params["profiles"].as_array().unwrap() {
        let name = entry["profile"].as_str().unwrap();
        let env = entry["invocation"]["env"].as_object().unwrap();
        let tools = served(&launch(env));
        assert!(!tools.is_empty(), "profile {name} served nothing");
        sets.insert(name.to_string(), tools);
    }
    // The bare invocation (no env) launches defaultProfile.
    let default = params["defaultProfile"].as_str().unwrap();
    assert_eq!(served(&launch(&serde_json::Map::new())), sets[default]);
    // Every profile serves the default surface; `full` serves the
    // union of all of them.
    let union: BTreeSet<String> = sets.values().flatten().cloned().collect();
    assert_eq!(sets["full"], union);
    for (name, tools) in &sets {
        assert!(tools.is_superset(&sets[default]), "profile {name}");
    }
}

// ---------- adversarial ----------

#[test]
fn adv_a_card_profile_missing_from_the_catalog_fails() {
    let mut card = read_json(".well-known/agent-card.json");
    let declared = declared_profiles();
    let mut ghost = profile_entries(&mut card)[0].clone();
    ghost["profile"] = "ghost".into();
    ghost["invocation"]["env"] = serde_json::json!({PROFILE_ENV: "ghost"});
    profile_entries(&mut card).push(ghost.clone());
    assert_eq!(
        check_card_profiles(&card, &declared),
        Err("profile ghost is not declared by the server".into())
    );
    // And the server itself refuses that entry's invocation.
    let output = launch(ghost["invocation"]["env"].as_object().unwrap());
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn adv_a_card_with_a_duplicate_profile_fails() {
    let mut card = read_json(".well-known/agent-card.json");
    let declared = declared_profiles();
    let twin = profile_entries(&mut card)[2].clone();
    let name = twin["profile"].as_str().unwrap().to_string();
    profile_entries(&mut card).push(twin);
    assert_eq!(
        check_card_profiles(&card, &declared),
        Err(format!("duplicate profile {name}"))
    );
}

#[test]
fn adv_a_card_that_drops_reorders_or_miswires_a_profile_fails() {
    let pristine = read_json(".well-known/agent-card.json");
    let declared = declared_profiles();
    // A declared profile dropped from the card.
    let mut card = pristine.clone();
    let dropped = profile_entries(&mut card).remove(3);
    assert_eq!(
        check_card_profiles(&card, &declared),
        Err(format!(
            "declared profile {} is missing from the card",
            dropped["profile"].as_str().unwrap()
        ))
    );
    // Out of declared order.
    let mut card = pristine.clone();
    profile_entries(&mut card).swap(0, 1);
    assert!(check_card_profiles(&card, &declared).is_err());
    // An entry whose env selects a different profile than it names.
    let mut card = pristine.clone();
    profile_entries(&mut card)[1]["invocation"]["env"] = serde_json::json!({PROFILE_ENV: "full"});
    assert!(check_card_profiles(&card, &declared).is_err());
    // An entry smuggling a second environment variable.
    let mut card = pristine.clone();
    profile_entries(&mut card)[1]["invocation"]["env"]["EXTRA"] = "1".into();
    assert!(check_card_profiles(&card, &declared).is_err());
    // An entry that launches something other than the base command.
    let mut card = pristine.clone();
    profile_entries(&mut card)[1]["invocation"]["command"] = "/elsewhere/meister".into();
    assert!(check_card_profiles(&card, &declared).is_err());
    // No profiles at all, and a default that names nothing listed.
    let mut card = pristine.clone();
    card["capabilities"]["extensions"][0]["params"]["defaultProfile"] = "ghost".into();
    assert!(check_card_profiles(&card, &declared).is_err());
    let mut card = pristine;
    card["capabilities"]["extensions"][0]["params"]
        .as_object_mut()
        .unwrap()
        .remove("profiles");
    assert!(check_card_profiles(&card, &declared).is_err());
}
