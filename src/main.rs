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
//! about and ignored).

#![forbid(unsafe_code)]

mod catalog;
mod guard;
mod hash;
mod lane;
mod server;
mod store;

use std::process::ExitCode;
use std::sync::Arc;

use rmcp::ServiceExt;
use rmcp::transport::async_rw::AsyncRwTransport;
use tokio::sync::Mutex;

/// Duplex buffer between the era guard and the SDK service. Request
/// lines can reach the 4 MiB line bound; the SDK consumes
/// incrementally, so this only needs to cover response bursts.
const DUPLEX_BUFFER_BYTES: usize = 1024 * 1024;

/// Wire the pieces together: the guard owns stdio, the SDK service
/// sits behind an in-process duplex, and a pump copies its responses
/// back to stdout through the same lock the guard writes with.
async fn run() -> std::io::Result<()> {
    let (client_side, server_side) = tokio::io::duplex(DUPLEX_BUFFER_BYTES);
    let (server_read, server_write) = tokio::io::split(server_side);
    let (client_read, client_write) = tokio::io::split(client_side);
    let stdout: guard::SharedStdout = Arc::new(Mutex::new(tokio::io::stdout()));

    let ordering = guard::Ordering::new();
    let handler = server::MeisterServer::new();
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
    match runtime.block_on(run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("meister: serve loop ended: {e}");
            ExitCode::FAILURE
        }
    }
}
