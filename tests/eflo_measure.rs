//! EFLO-01: the measurement script, driven against the real binary
//! and against stand-in binaries that answer badly.
//!
//! `scripts/eflo_measure.py` captures `tools/list` over stdio and
//! counts real tokens (`tiktoken`, cl100k_base). Its contract is
//! that a surface it cannot read is an ERROR, never "0 tokens", and
//! that the report is a pure function of the served surface. These
//! tests need `python3` with `tiktoken` on the machine: per the
//! plan of record no estimate is substituted, so a missing
//! tokenizer fails the gate rather than being skipped.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

/// The tests here write small executables and then run them. Held
/// for a whole test, this lock keeps another test's process spawn
/// from inheriting a stand-in's write handle (the ETXTBSY race).
static SERIAL: Mutex<()> = Mutex::new(());

static DIR_SEQ: AtomicU64 = AtomicU64::new(0);

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn fresh_dir(tag: &str) -> PathBuf {
    let seq = DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "meister-eflo-test-{tag}-{}-{seq}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run the script in isolated mode against `binary`, writing `out`,
/// with `MEISTER_PROFILE` unset: the default surface.
fn measure(binary: &Path, out: &Path) -> Output {
    measure_profiles(binary, out, &[])
}

/// As `measure`, with one `--profile` per name.
fn measure_profiles(binary: &Path, out: &Path, profiles: &[&str]) -> Output {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/eflo_measure.py");
    let mut command = Command::new("python3");
    command
        .arg("-I")
        .arg(&script)
        .arg("--binary")
        .arg(binary)
        .arg("--out")
        .arg(out)
        .env_remove("MEISTER_PROFILE");
    for profile in profiles {
        command.arg("--profile").arg(profile);
    }
    command.output().expect("python3 runs")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A stand-in server: drains stdin, then prints `frames` verbatim.
fn stand_in(dir: &Path, name: &str, frames: &[&str]) -> PathBuf {
    let mut script = String::from("#!/bin/sh\ncat >/dev/null\n");
    for frame in frames {
        assert!(!frame.contains('\''), "stand-in frames are single-quoted");
        script.push_str(&format!("printf '%s\\n' '{frame}'\n"));
    }
    let path = dir.join(name);
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

const DISCOVER_OK: &str = r#"{"jsonrpc":"2.0","id":0,"result":{"_meta":{"io.modelcontextprotocol/serverInfo":{"name":"stand-in","version":"0"}}}}"#;

/// The integer cells of the first per-profile table row:
/// (tools, bytes, tokens).
fn profile_row(report: &str) -> (u64, u64, u64) {
    let row = report
        .lines()
        .skip_while(|line| !line.starts_with("| Profile |"))
        .nth(2)
        .expect("a per-profile row");
    let cells: Vec<u64> = row
        .split('|')
        .filter_map(|cell| {
            let digits: String = cell
                .chars()
                .filter(|c| !matches!(c, ',' | '*' | ' '))
                .collect();
            digits.parse().ok()
        })
        .collect();
    (cells[0], cells[1], cells[2])
}

// ---------- validation ----------

#[test]
fn measuring_the_same_binary_twice_is_byte_identical() {
    let _guard = serial();
    let dir = fresh_dir("twice");
    let bin = Path::new(env!("CARGO_BIN_EXE_meister"));
    let (first, second) = (dir.join("first.md"), dir.join("second.md"));
    for out in [&first, &second] {
        let run = measure(bin, out);
        assert!(run.status.success(), "measure failed: {}", stderr_of(&run));
    }
    let (a, b) = (
        std::fs::read(&first).unwrap(),
        std::fs::read(&second).unwrap(),
    );
    assert!(!a.is_empty());
    assert_eq!(a, b, "two measurements of one binary differ");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn report_counts_the_served_surface_per_profile_and_per_tool() {
    let _guard = serial();
    let dir = fresh_dir("report");
    let out = dir.join("EFLO.md");
    let run = measure(Path::new(env!("CARGO_BIN_EXE_meister")), &out);
    assert!(run.status.success(), "measure failed: {}", stderr_of(&run));
    let report = std::fs::read_to_string(&out).unwrap();
    assert!(report.contains("cl100k_base"));
    let (tools, bytes, tokens) = profile_row(&report);
    // One per-tool row for every tool the profile row claims.
    let tool_rows = report
        .lines()
        .filter(|line| line.starts_with("| `meister_"))
        .count() as u64;
    assert!(tools > 0);
    assert_eq!(tool_rows, tools, "per-tool rows / profile tool count");
    assert!(report.contains("| `meister_lane_check` |"));
    // Real counts: a token is at least one byte, and this surface
    // is not empty.
    assert!(tokens > 0 && tokens <= bytes, "{tokens} tokens / {bytes} B");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_named_profile_is_measured_in_its_own_session() {
    let _guard = serial();
    let dir = fresh_dir("profiles");
    let out = dir.join("EFLO.md");
    let bin = Path::new(env!("CARGO_BIN_EXE_meister"));
    let run = measure_profiles(bin, &out, &["core", "full"]);
    assert!(run.status.success(), "measure failed: {}", stderr_of(&run));
    let report = std::fs::read_to_string(&out).unwrap();
    // Two profile rows, in the order asked; the first is core.
    let rows: Vec<&str> = report
        .lines()
        .skip_while(|line| !line.starts_with("| Profile |"))
        .skip(2)
        .take_while(|line| line.starts_with('|'))
        .collect();
    assert_eq!(rows.len(), 2, "rows: {rows:?}");
    assert!(rows[0].starts_with("| `core` | 4 |"), "row: {}", rows[0]);
    assert!(rows[1].starts_with("| `full` | 31 |"), "row: {}", rows[1]);
    // The script selects the profile itself: a caller's own
    // MEISTER_PROFILE does not leak into a named session.
    let leaked = dir.join("leaked.md");
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/eflo_measure.py");
    let run = Command::new("python3")
        .arg("-I")
        .arg(&script)
        .arg("--binary")
        .arg(bin)
        .arg("--out")
        .arg(&leaked)
        .args(["--profile", "core", "--profile", "full"])
        .env("MEISTER_PROFILE", "release")
        .output()
        .expect("python3 runs");
    assert!(run.status.success(), "measure failed: {}", stderr_of(&run));
    assert_eq!(
        std::fs::read(&leaked).unwrap(),
        std::fs::read(&out).unwrap()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- adversarial ----------

#[test]
fn adv_an_undeclared_profile_is_an_error_not_a_measurement() {
    let _guard = serial();
    let dir = fresh_dir("undeclared");
    let out = dir.join("EFLO.md");
    let bin = Path::new(env!("CARGO_BIN_EXE_meister"));
    // The server refuses to start; nothing is measured, and the
    // declared profile asked alongside it is not written either.
    let run = measure_profiles(bin, &out, &["core", "nope"]);
    assert_eq!(run.status.code(), Some(1));
    let stderr = stderr_of(&run);
    assert!(stderr.contains("profile nope"), "stderr: {stderr}");
    assert!(stderr.contains("unknown_profile"), "stderr: {stderr}");
    assert!(!out.exists(), "a failed measurement wrote a report");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn adv_malformed_json_is_an_error_not_zero_tokens() {
    let _guard = serial();
    let dir = fresh_dir("malformed");
    let bin = stand_in(&dir, "garbage", &["this is not json"]);
    let out = dir.join("EFLO.md");
    let run = measure(&bin, &out);
    assert_eq!(run.status.code(), Some(1), "malformed JSON must fail");
    assert!(
        stderr_of(&run).contains("malformed JSON"),
        "stderr: {}",
        stderr_of(&run)
    );
    assert!(!out.exists(), "a failed measurement wrote a report");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn adv_unreadable_surfaces_are_errors_and_leave_the_report_alone() {
    let _guard = serial();
    let dir = fresh_dir("unreadable");
    let out = dir.join("EFLO.md");
    let sentinel = b"previous report\n";
    std::fs::write(&out, sentinel).unwrap();
    let cases: [(&str, &[&str], &str); 5] = [
        // Valid JSON, empty surface: not "0 tokens".
        (
            "empty",
            &[
                DISCOVER_OK,
                r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#,
            ],
            "served no tools",
        ),
        // tools/list answered with a protocol error.
        (
            "refused",
            &[
                DISCOVER_OK,
                r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32022,"message":"no"}}"#,
            ],
            "answered an error",
        ),
        // tools/list never answered.
        ("silent", &[DISCOVER_OK], "no answer to tools/list"),
        // A page of a larger surface is not the surface.
        (
            "paged",
            &[
                DISCOVER_OK,
                r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"t"}],"nextCursor":"more"}}"#,
            ],
            "paginated",
        ),
        // A valid frame followed by a truncated one.
        (
            "truncated",
            &[
                DISCOVER_OK,
                r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[{"#,
            ],
            "malformed JSON",
        ),
    ];
    for (name, frames, expected) in cases {
        let bin = stand_in(&dir, name, frames);
        let run = measure(&bin, &out);
        assert_eq!(run.status.code(), Some(1), "{name} must fail");
        assert!(
            stderr_of(&run).contains(expected),
            "{name} stderr: {}",
            stderr_of(&run)
        );
        assert_eq!(
            std::fs::read(&out).unwrap(),
            sentinel,
            "{name} touched the previous report"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn adv_a_failing_or_missing_binary_is_an_error() {
    let _guard = serial();
    let dir = fresh_dir("failing");
    let out = dir.join("EFLO.md");
    // Exits non-zero after answering nothing.
    let failing = dir.join("failing");
    std::fs::write(&failing, "#!/bin/sh\ncat >/dev/null\nexit 3\n").unwrap();
    std::fs::set_permissions(&failing, std::fs::Permissions::from_mode(0o755)).unwrap();
    let run = measure(&failing, &out);
    assert_eq!(run.status.code(), Some(1));
    assert!(stderr_of(&run).contains("server exited 3"));
    // Not there at all.
    let run = measure(&dir.join("absent"), &out);
    assert_eq!(run.status.code(), Some(1));
    assert!(stderr_of(&run).contains("cannot run"));
    assert!(!out.exists(), "a failed measurement wrote a report");
    let _ = std::fs::remove_dir_all(&dir);
}
