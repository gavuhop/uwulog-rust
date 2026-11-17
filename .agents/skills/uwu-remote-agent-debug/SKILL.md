---
name: uwu-remote-agent-debug
description: >-
  Debug, test, and develop uwu-agent, framed RPC protocol envelopes (core_protocol),
  WSL/SSH transports (driver_transport), and ingestion sources (driver_sources). Activate when
  working on remote log streaming, AGENT_READY_MARKER handshake, file tailing/rotation, or
  child process tree management.
---

# 📡 `uwu-remote-agent-debug` — Remote Agent & Transport Runbook

---

## 1. Architecture & Protocol Contract

- **Framing (`crates/core_protocol/src/lib.rs`)**:
  - `AGENT_READY_MARKER` = `b"__UWU_AGENT_READY__\n"`.
  - On startup in `uwu-agent proxy`, `uwu-agent` writes `AGENT_READY_MARKER` to `stdout`, then switches to 4-byte Big-Endian length-prefixed JSON envelopes (`MAX_FRAME_SIZE = 64 MB`) via `FramedReader` / `FramedWriter`.
  - `RemoteSource` (`crates/driver_sources/src/remote.rs`) calls `uwu_core_protocol::wait_for_ready_marker(&mut reader)` with a `15s` timeout to discard SSH MOTD, banners, and `.bashrc` output before reading envelopes.
- **Diagnostics Separation**: `uwu-agent proxy` MUST keep `stdout` strictly for `AGENT_READY_MARKER` + framed binary envelopes; diagnostics and panic hooks go to `stderr` (`setup_proxy_diagnostics`).
- **Envelopes**:
  - Client $\rightarrow$ Server (`ClientEnvelope`): `Ping { seq }`, `StartStream { spec, filter_pushdown }`, `PauseStream`, `ResumeStream`, `StopStream`.
  - Server $\rightarrow$ Client (`ServerEnvelope`): `Pong { seq }`, `LogBatch(Vec<RawLogEntry>)`, `Error { message }`, `Terminated { exit_code }`.

---

## 2. Driver Invariants (`driver_sources` & `driver_transport`)

- **Tier 1 Boundary**: Never import `uwu-core-engine` or `uwu-core-filter`. `uwu-driver-transport` depends only on `uwu-core-util`.
- **File Tailing (`file_tailer.rs`)**: Preserve `file_len < current_pos` truncation/rotation detection (`SeekFrom::Start(0)`) and fallback ticker alongside `notify::RecommendedWatcher`.
- **Process Tree Cleanup (`process.rs`)**: Preserve `cmd.kill_on_drop(true)`, Windows Job Objects (`setup_windows_job_object`), and `tx.closed()` termination handling.

---

## 3. Loopback Verification with `uwu-agent`

Run the included loopback harness script against the local `uwu-agent` binary to verify handshake, `Ping`/`Pong`, file streaming (`StartStream` -> `LogBatch`), and `StopStream`:

```bash
python3 .agents/skills/uwu-remote-agent-debug/scripts/test_agent_loopback.py
```

And verify Tier 1 & Tier 4 tests:
```bash
./.agents/scripts/harness.sh check-arch
./.agents/scripts/harness.sh test-tier 0    # Tests core_protocol framed envelope & MOTD skip
./.agents/scripts/harness.sh test-tier 1    # Tests driver_sources & driver_transport
./.agents/scripts/harness.sh test-tier 4    # Tests uwu-cli-agent
```
