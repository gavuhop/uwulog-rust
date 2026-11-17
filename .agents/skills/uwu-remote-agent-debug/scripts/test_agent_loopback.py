#!/usr/bin/env python3
"""
Loopback integration verifier for `uwu-agent proxy`:
  1. Builds (if needed) or locates `uwu-agent`
  2. Spawns `uwu-agent proxy` over stdio pipes
  3. Verifies `AGENT_READY_MARKER` (`__UWU_AGENT_READY__\n`)
  4. Sends 4-byte BE framed `ClientEnvelope::Ping` -> expects `ServerEnvelope::Pong`
  5. Sends `ClientEnvelope::StartStream` on a temp log file -> expects `ServerEnvelope::LogBatch`
  6. Sends `ClientEnvelope::StopStream` and shuts down cleanly
"""

from __future__ import annotations

import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path

READY_MARKER = b"__UWU_AGENT_READY__\n"


def find_repo_root() -> Path:
    cur = Path(__file__).resolve()
    for p in cur.parents:
        if (p / "Cargo.toml").is_file() and (p / "crates").is_dir():
            return p
    return Path.cwd()


def resolve_agent_bin(repo_root: Path) -> Path:
    exe_suffix = ".exe" if os.name == "nt" else ""
    for profile in ("debug", "release-dev", "release"):
        candidate = repo_root / "target" / profile / f"uwu-agent{exe_suffix}"
        if candidate.is_file():
            return candidate

    print("[loopback] Building uwu-agent (debug)...")
    res = subprocess.run(
        ["cargo", "build", "-p", "uwu-cli-agent", "--bin", "uwu-agent"],
        cwd=repo_root,
        check=False,
    )
    if res.returncode != 0:
        raise RuntimeError("Failed to build uwu-agent")
    return repo_root / "target" / "debug" / f"uwu-agent{exe_suffix}"


def send_frame(stream, payload_obj: dict) -> None:
    raw = json.dumps(payload_obj).encode("utf-8")
    stream.write(struct.pack(">I", len(raw)) + raw)
    stream.flush()


def read_frame(stream) -> dict:
    hdr = stream.read(4)
    if len(hdr) < 4:
        raise EOFError(f"Expected 4-byte frame header, got {len(hdr)} bytes")
    (length,) = struct.unpack(">I", hdr)
    if length > 64 * 1024 * 1024:
        raise ValueError(f"Frame length {length} exceeds MAX_FRAME_SIZE")
    body = stream.read(length)
    if len(body) < length:
        raise EOFError(f"Truncated frame body: expected {length}, got {len(body)}")
    return json.loads(body.decode("utf-8"))


def main() -> int:
    repo_root = find_repo_root()
    agent_bin = resolve_agent_bin(repo_root)
    print(f"[loopback] Using binary: {agent_bin}")

    tmp_dir = Path(tempfile.mkdtemp(prefix="uwu_agent_loopback_"))
    log_file = tmp_dir / "sample.log"
    log_file.write_text(
        '{"level":"INFO","message":"loopback boot"}\n'
        '{"level":"ERROR","message":"loopback probe error"}\n',
        encoding="utf-8",
    )

    proc = subprocess.Popen(
        [str(agent_bin), "proxy", "-d", str(tmp_dir)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    try:
        assert proc.stdout is not None and proc.stdin is not None
        # 1. Wait for AGENT_READY_MARKER
        buf = bytearray()
        deadline = time.monotonic() + 10.0
        while time.monotonic() < deadline:
            b = proc.stdout.read(1)
            if not b:
                err_txt = proc.stderr.read().decode("utf-8", errors="replace") if proc.stderr else ""
                raise RuntimeError(f"uwu-agent exited before ready marker: {err_txt}")
            buf.extend(b)
            if buf.endswith(READY_MARKER):
                break
        else:
            raise TimeoutError("Timed out waiting for AGENT_READY_MARKER")
        print("[loopback] Handshake OK: received __UWU_AGENT_READY__")

        # 2. Ping / Pong
        send_frame(proc.stdin, {"Ping": {"seq": 42}})
        pong = read_frame(proc.stdout)
        if pong != {"Pong": {"seq": 42}}:
            raise AssertionError(f"Unexpected Ping response: {pong}")
        print("[loopback] Heartbeat OK: Pong { seq: 42 }")

        # 3. StartStream on temp file
        send_frame(
            proc.stdin,
            {
                "StartStream": {
                    "spec": {"File": "sample.log"},
                    "filter_pushdown": None,
                }
            },
        )

        collected = []
        for _ in range(10):
            frame = read_frame(proc.stdout)
            if "LogBatch" in frame:
                collected.extend(frame["LogBatch"])
                if len(collected) >= 2:
                    break
            elif "Error" in frame:
                raise AssertionError(f"Agent returned error envelope: {frame}")

        if len(collected) < 2:
            raise AssertionError(f"Expected >= 2 log entries, got {len(collected)}: {collected}")
        print(f"[loopback] Stream OK: received {len(collected)} RawLogEntry records")

        # 4. StopStream + close stdin
        send_frame(proc.stdin, "StopStream")
        proc.stdin.close()
        rc = proc.wait(timeout=5)
        if rc != 0:
            err_txt = proc.stderr.read().decode("utf-8", errors="replace") if proc.stderr else ""
            raise RuntimeError(f"uwu-agent exited with non-zero code {rc}: {err_txt}")

        print("[loopback] PASS: all protocol loopback checks succeeded.")
        return 0
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait(timeout=5)
        shutil.rmtree(tmp_dir, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
