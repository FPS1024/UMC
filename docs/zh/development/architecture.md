---
translation: pending
source: en/development/architecture.md
---

# UMC architecture (draft v0.1)

## Product boundary

UMC is a headless launcher engine. Community applications own UI and call either the Rust SDK or the standalone process. Both paths enter `umc-core`, which is the only owner of launcher operations and policies.

```text
GUI / scripts ── Rust SDK ──┐
GUI / scripts ── JSON-RPC ──┼── umcd ── umc-core ── operating system, filesystem, network
Terminal ─────── CLI ───────┘
```

## Workspace now

| Crate | Responsibility |
|---|---|
| `umc-api` | Stable serializable contracts independent of transport and implementation |
| `umc-core` | Shared application services; vanilla installation, instance persistence, Java detection, launch and process supervision |
| `umc-sdk` | Stable Rust facade delegating to Core |
| `umc` | CLI and stdio JSON-RPC adapter |
| `umcd` | Local Unix Socket / Windows Named Pipe JSON-RPC host |
| `umc-loader-api` | Common loader discovery and installation preflight contract |
| `umc-loader-fabric` | Fabric Meta version/profile adapter |
| `umc-loader-forge` | Forge Maven metadata and installer artifact adapter |
| `umc-loader-neoforge` | NeoForge Maven metadata and installer artifact adapter |
| `umc-loader-registry` | Built-in and host-registered adapter composition for Core |

Loader adapters are separate crates because their release metadata and installer strategies differ. The registry is the Core-facing composition point. Loader crates depend on the public loader contract and their upstream metadata sources; they do not depend on Core, RPC, or GUI code. Rust SDK hosts can register trusted in-process implementations with `LauncherBuilder::register_loader`; daemon hosts configure their own registry. UMC does not download or dynamically load provider code. The current adapter slice discovers versions and produces plans; Core does not yet execute those plans.

## Planned Core modules

`version` resolves Mojang manifests and version documents; `download` streams files, verifies expected hashes, and reports progress; `storage` owns data layout and path confinement; `java` discovers compatible runtimes; `instance` manages isolated installations; `launch` assembles arguments and supervises child processes; `auth` integrates supported account flows and OS credential stores; `task` persists task state and emits sequenced events. Interfaces should be introduced where a real alternate implementation is needed, not preemptively across every function.

## Public interface policy

Rust SDK uses SemVer. JSON-RPC uses an independent version (`v1`) advertised through `system.info` and `system.capabilities`. RPC error codes and event envelopes are stable contracts. Additive fields are optional; clients ignore unknown fields and events. Deprecate before removal. Never expose internal Rust error strings as protocol codes without mapping.

## Data and security boundaries

Default managed data root is `UMC_DATA_DIR` or `./umc-data` during development; production platform-specific defaults remain to be specified. Instance operations must canonicalize and confine paths beneath the selected root. Tokens belong in OS credential storage, never ordinary instance JSON or logs. RPC never offers arbitrary shell execution. The daemon uses local-only IPC: Unix Socket mode `0600` under a mode `0700` data directory, and a Windows Named Pipe with an owner-only ACL that rejects remote clients. It opens no TCP listener.

## 0.1.0 delivery sequence

1. Workspace, typed contracts, structured errors, config and CLI foundation.
2. Official version metadata, downloads with hash checks, Java detection, vanilla installation and launch.
3. Isolated instances and stable instance/launch request models.
4. RPC stdio, task/event semantics, and SDK parity.
5. Local daemon IPC with process reconnection and state recovery.
6. Authentication and loader/modpack integrations.

The workspace now includes the vanilla flow and initial loader adapters: official version metadata, SHA-1/size checked assets/libraries/client downloads, Java major-version compatibility detection, opt-in Mojang runtime provisioning, isolated instance files, launch/process status/stop, CLI and Rust SDK access, and Fabric/Forge/NeoForge version discovery plus preflight planning. Managed runtimes live below `runtimes/` and are selected when their component matches the Minecraft version metadata. Instance config schema v1 migrates legacy files, keeps a backup on write, quarantines corruption, and offers repair from a validated install record. `umcd` serves the same JSON-RPC contract locally, and the SDK can reconnect through its socket or pipe. Install operations use persistent tasks with progress events, cancellation, retry, timeout, and shared concurrency limits. Process records are recovered on startup and stopped PIDs are identity-checked before signaling. See RPC v1 for current loader limits and task recovery semantics.

