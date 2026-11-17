#!/usr/bin/env python3
"""EFLO measurement: the real token cost of meister's served tools/list.

Run with `python3 -I scripts/eflo_measure.py` (isolated mode), from
anywhere; paths default relative to the repository root.

Method (the method of lumen `EFLO.md`): spawn the built binary, open a
modern-era stdio session (`server/discover`, then `tools/list`, each
request carrying the 2026-07-28 `_meta` probe), take the served `tools`
array from the wire, canonicalise it as compact JSON with sorted keys,
and count tokens with the real tokenizer (`tiktoken`, cl100k_base).
One session per profile: the profile is selected the way a client
selects it, through the `MEISTER_PROFILE` environment variable.

Contract:

- No estimate is ever substituted. If `tiktoken` or its cl100k_base
  encoding is unavailable, the run stops with an error.
- A surface that cannot be read is an error, never "0 tokens": a
  malformed frame, a missing or error `tools/list` answer, a non-zero
  server exit, an empty tool list, or a paginated answer each fail the
  whole run with exit code 1.
- The output file is written only after every profile measured clean,
  and written atomically; a failed run leaves the previous file as it
  was.
- The output is a pure function of the served surface and the
  tokenizer: no dates, no host paths. Measuring the same binary twice
  gives a byte-identical file.

Profiles: each `--profile NAME` measures one session launched with
`MEISTER_PROFILE=NAME`. With no `--profile`, the profile named by the
caller's own `MEISTER_PROFILE` is measured; with neither, the server's
default surface is measured with `MEISTER_PROFILE` unset.
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile

# The fleet metadata budget (decision D5): tokens, cl100k_base.
BUDGET_TOKENS = 2500
ENCODING_NAME = "cl100k_base"
PROFILE_ENV = "MEISTER_PROFILE"
PROTOCOL_VERSION = "2026-07-28"
# Bounds: one session is two requests against a local binary.
SESSION_TIMEOUT_SECONDS = 30
MAX_PROFILES = 32
# Label for the surface served when MEISTER_PROFILE is unset.
DEFAULT_LABEL = "(default)"

DISCOVER_ID = 0
LIST_ID = 1

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


class MeasureError(Exception):
    """A surface could not be measured. Never reported as a count."""


def meta():
    return {
        "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
        "io.modelcontextprotocol/clientInfo": {"name": "eflo-measure", "version": "1"},
        "io.modelcontextprotocol/clientCapabilities": {},
    }


def request(request_id, method):
    frame = {"jsonrpc": "2.0", "id": request_id, "method": method, "params": {"_meta": meta()}}
    return json.dumps(frame, separators=(",", ":"))


def canonical(value):
    """Compact JSON, sorted keys, raw UTF-8 (as the wire serves it)."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def capture(binary, profile):
    """Run one session; return (server version, served tools array)."""
    label = profile if profile is not None else DEFAULT_LABEL
    env = dict(os.environ)
    env.pop(PROFILE_ENV, None)
    if profile is not None:
        env[PROFILE_ENV] = profile
    stdin = request(DISCOVER_ID, "server/discover") + "\n" + request(LIST_ID, "tools/list") + "\n"
    try:
        done = subprocess.run(
            [binary],
            input=stdin.encode("utf-8"),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            timeout=SESSION_TIMEOUT_SECONDS,
            check=False,
        )
    except OSError as e:
        raise MeasureError(f"profile {label}: cannot run {binary}: {e}") from e
    except subprocess.TimeoutExpired as e:
        raise MeasureError(
            f"profile {label}: no answer within {SESSION_TIMEOUT_SECONDS}s"
        ) from e
    if done.returncode != 0:
        detail = done.stderr.decode("utf-8", "replace").strip()
        raise MeasureError(f"profile {label}: server exited {done.returncode}: {detail}")

    try:
        text = done.stdout.decode("utf-8")
    except UnicodeDecodeError as e:
        raise MeasureError(f"profile {label}: stdout is not UTF-8: {e}") from e
    frames = {}
    for number, line in enumerate(text.splitlines(), start=1):
        if not line.strip():
            continue
        try:
            frame = json.loads(line)
        except json.JSONDecodeError as e:
            raise MeasureError(f"profile {label}: malformed JSON on stdout line {number}: {e}") from e
        if not isinstance(frame, dict):
            raise MeasureError(f"profile {label}: stdout line {number} is not a JSON object")
        frame_id = frame.get("id")
        if isinstance(frame_id, int) and not isinstance(frame_id, bool):
            frames[frame_id] = frame

    results = {}
    for frame_id, method in ((DISCOVER_ID, "server/discover"), (LIST_ID, "tools/list")):
        frame = frames.get(frame_id)
        if frame is None:
            raise MeasureError(f"profile {label}: no answer to {method}")
        if "error" in frame:
            raise MeasureError(f"profile {label}: {method} answered an error: {canonical(frame['error'])}")
        result = frame.get("result")
        if not isinstance(result, dict):
            raise MeasureError(f"profile {label}: {method} answer carries no result object")
        results[frame_id] = result

    listed = results[LIST_ID]
    if listed.get("nextCursor") is not None:
        raise MeasureError(f"profile {label}: tools/list is paginated; a partial surface is not measured")
    tools = listed.get("tools")
    if not isinstance(tools, list) or not tools:
        raise MeasureError(f"profile {label}: tools/list served no tools")
    names = []
    for tool in tools:
        if not isinstance(tool, dict) or not isinstance(tool.get("name"), str):
            raise MeasureError(f"profile {label}: a served tool has no string name")
        names.append(tool["name"])
    if len(set(names)) != len(names):
        raise MeasureError(f"profile {label}: tools/list served a duplicate tool name")

    info = results[DISCOVER_ID].get("_meta", {}).get("io.modelcontextprotocol/serverInfo", {})
    version = info.get("version")
    if not isinstance(version, str) or not version:
        raise MeasureError(f"profile {label}: server/discover names no server version")
    return version, tools


def load_encoding():
    try:
        import tiktoken

        return tiktoken, tiktoken.get_encoding(ENCODING_NAME)
    except Exception as e:  # ImportError, or the encoding cannot be loaded
        raise MeasureError(
            f"tiktoken {ENCODING_NAME} is unavailable ({e}); no estimate is substituted"
        ) from e


def count(encoding, text):
    return len(encoding.encode(text))


def budget_cell(tokens):
    if tokens <= BUDGET_TOKENS:
        return f"within ({BUDGET_TOKENS - tokens:,} under)"
    return f"OVER by {tokens - BUDGET_TOKENS:,}"


def render(tiktoken_version, server_version, measured):
    """measured: list of (label, tools array, counts), in the order asked."""
    out = []
    out.append("# EFLO — tools/list measurement")
    out.append("")
    out.append("Generated by `scripts/eflo_measure.py` (`just eflo`). Do not edit by")
    out.append("hand; re-run the script.")
    out.append("")
    out.append(f"- Server: meister {server_version}, the built binary over stdio, one")
    out.append(f"  modern-era session per profile (`{PROFILE_ENV}`).")
    out.append("- Method: the served `tools` array, taken from the wire and")
    out.append("  canonicalised as compact JSON with sorted keys, raw UTF-8.")
    out.append(f"- Tokenizer: `tiktoken` {tiktoken_version}, `{ENCODING_NAME}`. Real counts; no")
    out.append("  estimate appears in this file.")
    out.append(f"- Budget (fleet EFLO, decision D5): **≤ {BUDGET_TOKENS:,} tokens** per served")
    out.append("  surface. The Budget column states the arithmetic only; a waiver")
    out.append("  or a split is the Owner's ruling, recorded in COMPLIANCE.md §3.")
    out.append("")
    out.append("## Per profile")
    out.append("")
    out.append("| Profile | Tools | Bytes | Tokens | Budget |")
    out.append("|---|---:|---:|---:|---|")
    for label, tools, row in measured:
        out.append(
            f"| `{label}` | {len(tools)} | {row['bytes']:,} | **{row['tokens']:,}** | {budget_cell(row['tokens'])} |"
        )
    out.append("")
    out.append(f"`{DEFAULT_LABEL}` is the surface served with `{PROFILE_ENV}` unset.")
    out.append("Bytes are the UTF-8 length of the canonical array.")
    out.append("")
    out.append("## Per tool")
    out.append("")
    out.append("Each tool object canonicalised and counted on its own. A profile's")
    out.append("total is counted over its whole array, so it differs from the sum of")
    out.append("its tools by the array punctuation.")
    for label, tools, row in measured:
        out.append("")
        out.append(f"### `{label}` — {len(tools)} tools, {row['tokens']:,} tokens")
        out.append("")
        out.append("| Tool | Bytes | Tokens |")
        out.append("|---|---:|---:|")
        for name, tool_bytes, tool_tokens in row["tools"]:
            out.append(f"| `{name}` | {tool_bytes:,} | {tool_tokens:,} |")
    out.append("")
    return "\n".join(out)


def write_atomic(path, text):
    directory = os.path.dirname(os.path.abspath(path))
    handle, temp = tempfile.mkstemp(dir=directory, prefix=".eflo-", suffix=".tmp")
    try:
        with os.fdopen(handle, "w", encoding="utf-8") as f:
            f.write(text)
        os.replace(temp, path)
    except BaseException:
        if os.path.exists(temp):
            os.unlink(temp)
        raise


def measure(binary, profiles):
    tiktoken, encoding = load_encoding()
    measured = []
    versions = set()
    for profile in profiles:
        label = profile if profile is not None else DEFAULT_LABEL
        version, tools = capture(binary, profile)
        versions.add(version)
        array = canonical(tools)
        per_tool = []
        for tool in tools:
            text = canonical(tool)
            per_tool.append((tool["name"], len(text.encode("utf-8")), count(encoding, text)))
        # Largest first; name breaks ties so the order is total.
        per_tool.sort(key=lambda entry: (-entry[2], entry[0]))
        row = {
            "bytes": len(array.encode("utf-8")),
            "tokens": count(encoding, array),
            "tools": per_tool,
        }
        if row["tokens"] <= 0:
            raise MeasureError(f"profile {label}: counted {row['tokens']} tokens for a non-empty surface")
        measured.append((label, tools, row))
    if len(versions) != 1:
        raise MeasureError(f"sessions disagree on the server version: {sorted(versions)}")
    return render(tiktoken.__version__, versions.pop(), measured)


def main(argv):
    parser = argparse.ArgumentParser(description="Measure meister's served tools/list in real tokens.")
    parser.add_argument(
        "--binary",
        default=os.path.join(REPO_ROOT, "target", "debug", "meister"),
        help="the built meister binary (default: target/debug/meister)",
    )
    parser.add_argument(
        "--out",
        default=os.path.join(REPO_ROOT, "EFLO.md"),
        help="the report to write (default: EFLO.md at the repository root)",
    )
    parser.add_argument(
        "--profile",
        action="append",
        default=[],
        metavar="NAME",
        help=f"measure a session launched with {PROFILE_ENV}=NAME (repeatable)",
    )
    args = parser.parse_args(argv)

    profiles = list(args.profile)
    if not profiles:
        inherited = os.environ.get(PROFILE_ENV)
        profiles = [inherited] if inherited is not None else [None]
    if len(profiles) > MAX_PROFILES:
        print(f"eflo_measure: error: more than {MAX_PROFILES} profiles asked", file=sys.stderr)
        return 1
    if len(set(profiles)) != len(profiles):
        print("eflo_measure: error: a profile is named twice", file=sys.stderr)
        return 1

    try:
        report = measure(args.binary, profiles)
        write_atomic(args.out, report)
    except MeasureError as e:
        print(f"eflo_measure: error: {e}", file=sys.stderr)
        return 1
    except OSError as e:
        print(f"eflo_measure: error: cannot write {args.out}: {e}", file=sys.stderr)
        return 1
    print(f"eflo_measure: wrote {args.out} ({len(profiles)} profile(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
