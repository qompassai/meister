//! meister — MCP server for Qompass SFW game making (stdio).
//!
//! One maker surface for building SFW games end to end: project
//! scaffolding, cast and canon, authoring, asset routing,
//! playtest, and release handoff, all as structured game data with
//! provenance (plan of record: this repository's README, D-1604).
//!
//! Rung 0 (birth): the server speaks the MCP modern era only
//! (2026-07-28) on rmcp — sessions open with `server/discover` and
//! every request carries its protocol version in `_meta`; there is
//! no `initialize` handshake, and legacy contact is refused at the
//! stdio front door (see `guard`) with the typed `-32022`
//! unsupported-version error. The full tool surface is served as
//! typed seams; the lane gate (see `lane`) is live from this first
//! commit — Meister refuses NSFW and unclassified input before it
//! can author anything at all.
//!
//! Usage: `meister` (no arguments; unknown arguments are warned
//! about and ignored). The served surface profile is chosen at
//! launch by `MEISTER_PROFILE` (default `core`; see
//! `catalog::PROFILES`); an unknown value refuses to start.

#![forbid(unsafe_code)]

mod catalog;
mod graph;
mod guard;
mod hash;
mod lane;
mod pack;
mod server;
mod store;
mod teach;

use std::process::ExitCode;
use std::sync::Arc;

use rmcp::ServiceExt;
use rmcp::transport::async_rw::AsyncRwTransport;
use tokio::sync::Mutex;

/// Duplex buffer between the era guard and the SDK service. Request
/// lines can reach the 4 MiB line bound; the SDK consumes
/// incrementally, so this only needs to cover response bursts.
const DUPLEX_BUFFER_BYTES: usize = 1024 * 1024;

/// Exit status when `MEISTER_PROFILE` names no declared profile.
/// Distinct from the general failure status so a launcher can tell
/// a configuration refusal from a serve-loop fault.
const EXIT_UNKNOWN_PROFILE: u8 = 2;

/// Bound on how much of a refused `MEISTER_PROFILE` value is echoed.
const PROFILE_ECHO_MAX_CHARS: usize = 64;

/// Wire the pieces together: the guard owns stdio, the SDK service
/// sits behind an in-process duplex, and a pump copies its responses
/// back to stdout through the same lock the guard writes with.
async fn run(profile: &'static catalog::Profile) -> std::io::Result<()> {
    let (client_side, server_side) = tokio::io::duplex(DUPLEX_BUFFER_BYTES);
    let (server_read, server_write) = tokio::io::split(server_side);
    let (client_read, client_write) = tokio::io::split(client_side);
    let stdout: guard::SharedStdout = Arc::new(Mutex::new(tokio::io::stdout()));

    let ordering = guard::Ordering::new();
    let handler = server::MeisterServer::new(profile);
    let serve = tokio::spawn(async move {
        let transport = AsyncRwTransport::new_server(server_read, server_write);
        match handler.serve(transport).await {
            Ok(running) => running
                .waiting()
                .await
                .map(|_| ())
                .map_err(|e| std::io::Error::other(e.to_string())),
            // The client closed the connection before selecting a
            // lifecycle (a probe, a spawn check): bare EOF is a
            // clean end of session.
            Err(rmcp::service::ServerInitializeError::ConnectionClosed(_)) => Ok(()),
            Err(e) => Err(std::io::Error::other(e.to_string())),
        }
    });
    let mut pump = tokio::spawn(guard::pump(
        client_read,
        Arc::clone(&stdout),
        Arc::clone(&ordering),
    ));

    // When stdin reaches EOF the guard shuts the duplex write side
    // down, which ends the SDK service; the pump ends when the
    // service's transport drops. The pump wait is bounded as a
    // backstop so a wedged pump cannot hang exit.
    let guard_result = guard::run(tokio::io::stdin(), client_write, stdout, ordering).await;
    let serve_result = match serve.await {
        Ok(result) => result,
        Err(e) => Err(std::io::Error::other(e.to_string())),
    };
    if tokio::time::timeout(std::time::Duration::from_secs(2), &mut pump)
        .await
        .is_err()
    {
        pump.abort();
    }
    guard_result.and(serve_result)
}

fn main() -> ExitCode {
    for arg in std::env::args().skip(1) {
        eprintln!("meister: ignoring unknown arg {arg}");
    }
    // The profile declarations are checked before anything is
    // served: an inconsistent catalog refuses to start.
    if let Err(e) = catalog::check_declared_profiles() {
        eprintln!("meister: refusing to start: profile_catalog_invalid: {e}");
        return ExitCode::FAILURE;
    }
    // The compiled-in packs are validated before anything is
    // served (PK-01): a build carrying an invalid pack refuses to
    // start.
    if let Err(e) = pack::check_compiled() {
        eprintln!("meister: refusing to start: pack_catalog_invalid: {e}");
        return ExitCode::FAILURE;
    }
    // One profile per launch, fixed before anything is served. An
    // unknown value is a refusal, never a fallback to the default.
    let raw = std::env::var_os(catalog::ENV_PROFILE);
    let profile = match catalog::select_profile(raw.as_deref()) {
        Ok(profile) => profile,
        Err(given) => {
            let echoed: String = given.chars().take(PROFILE_ECHO_MAX_CHARS).collect();
            let declared: Vec<&str> = catalog::PROFILES.iter().map(|p| p.name).collect();
            eprintln!(
                "meister: refusing to start: unknown_profile: {}={echoed:?} is not a declared profile (declared: {})",
                catalog::ENV_PROFILE,
                declared.join(", ")
            );
            return ExitCode::from(EXIT_UNKNOWN_PROFILE);
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("meister: cannot start runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(profile)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("meister: serve loop ended: {e}");
            ExitCode::FAILURE
        }
    }
}
