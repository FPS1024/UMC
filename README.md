<div align="center">

<img src=".github/assets/appicon.png" width="96" alt="UMC app icon" />

<h1>Universal Minecraft Core</h1>

<p><strong>One Core. Every Launcher.</strong></p>

<p>
  A headless Minecraft: Java Edition launcher engine written in Rust.<br />
  Install, verify, launch, and supervise the game from your own application.
</p>

<p>
  <a href="https://github.com/FPS1024/UMC/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/FPS1024/UMC/ci.yml?branch=main&label=CI&style=flat-square" /></a>
  <a href="https://github.com/FPS1024/UMC/actions/workflows/docs.yml"><img alt="Documentation" src="https://img.shields.io/github/actions/workflow/status/FPS1024/UMC/docs.yml?branch=main&label=docs&style=flat-square" /></a>
  <a href="https://github.com/FPS1024/UMC/releases/latest"><img alt="Release" src="https://img.shields.io/github/v/release/FPS1024/UMC?style=flat-square&color=4f63e8" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/FPS1024/UMC?style=flat-square&color=22c997" /></a>
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.89%2B-000000?style=flat-square&logo=rust" />
  <img alt="Platform" src="https://img.shields.io/badge/platform-Linux%20x86__64-4f63e8?style=flat-square&logo=linux&logoColor=white" />
</p>

<p>
  <a href="https://fps1024.github.io/UMC/en/">Documentation</a> ·
  <a href="https://fps1024.github.io/UMC/en/guide/quickstart">Quickstart</a> ·
  <a href="https://fps1024.github.io/UMC/zh/">简体中文</a> ·
  <a href="https://github.com/FPS1024/UMC/releases/latest">Releases</a> ·
  <a href="https://discord.gg/d5qKvMp9Wc">Discord</a>
</p>

</div>

![UMC landing page](.github/assets/readme-preview.jpg)

## Why UMC?

UMC separates launcher functionality from the user interface. Your application owns the window,
accounts, and product experience; UMC owns version discovery, verified downloads, instances, tasks,
Java, launch, and process supervision.

One engine is available through four surfaces:

- **Rust SDK** — typed calls, in-process or through the local daemon.
- **Local daemon** — newline-delimited JSON-RPC over a private Unix socket or Windows named pipe.
- **CLI** — scripts, automation, and manual operations.
- **stdio JSON-RPC** — a child-process adapter for sandboxed integrations.

## Features

| | Capability | Details |
|---|---|---|
| 🦀 | **Rust-native core** | One `umc-core` implementation powers the SDK, CLI, stdio adapter, and daemon. |
| 🔒 | **Integrity checked** | Client JARs, libraries, assets, logs, and runtimes are verified by SHA-1 and size. |
| ⚙️ | **Persistent tasks** | Install operations expose sequenced progress events, cancellation, retries, timeouts, and restart recovery. |
| 🖥️ | **Local-first daemon** | Local-only IPC with owner-only permissions. No TCP listener, no account service, no telemetry. |
| ☕ | **Managed Java** | Detect compatible system Java or provision a verified Mojang runtime for a specific version. |
| 📦 | **Isolated instances** | Each instance has its own config, game directory, logs, and verified install record. |
| 🧩 | **Loader-ready adapters** | Fabric, Forge, and NeoForge version discovery and install preflight plans. |
| 🛠️ | **Repair and recovery** | Atomic config writes, backups, corruption quarantine, instance repair, and process state recovery. |

## Architecture

```text
GUI / scripts ── Rust SDK ──┐
GUI / scripts ── JSON-RPC ──┼── umc-core ── OS, filesystem, HTTPS
Terminal ─────── CLI ───────┘
```

`umc-core` is the only owner of launcher behavior. Every surface delegates to it, so task state,
instance data, errors, and process handling stay consistent.

## Project status

| Area | Status |
|---|---|
| Vanilla version discovery and verified downloads | ✅ Available |
| Isolated instance installation and repair | ✅ Available |
| Explicit offline launch and process supervision | ✅ Available |
| Java detection and managed Mojang runtimes | ✅ Available |
| Persistent tasks, local daemon, CLI, Rust SDK, JSON-RPC | ✅ Available |
| Fabric / Forge / NeoForge version discovery and preflight | ✅ Available |
| Microsoft account sign-in and vanilla online launch | ✅ Available |
| Loader installation and loader online launch | 🚧 In development |
| Mods, modpacks, and resource packs | 🚧 Planned |
| Windows and macOS ports | 🚧 Planned |

## Quickstart

Build from source:

```sh
git clone https://github.com/FPS1024/UMC.git
cd UMC
cargo build --workspace --locked --release

./target/release/umc version list
./target/release/umc instance create Survival 1.21.1
./target/release/umc instance install <instance-id>
./target/release/umc java install 1.21.1
./target/release/umc auth login
./target/release/umc launch <instance-id> --online
```

Offline launch is still available for local testing:

```sh
./target/release/umc launch <instance-id> --offline Steve
```

Or download the latest Linux archive from
[GitHub Releases](https://github.com/FPS1024/UMC/releases/latest) and verify its SHA-256 checksum.

Read the full [Quickstart](https://fps1024.github.io/UMC/en/guide/quickstart) for release setup,
Java provisioning, logs, and first-run details.

> [!WARNING]
> Offline launch does not verify account ownership and cannot join authenticated servers. Use `umc auth login` plus `umc launch --online` for online play. UMC stores only the Microsoft refresh token in the OS keyring (or a `0600` file fallback) and never stores your password.

## Optional companion services

UMC also ships independently deployable services under [`services/`](services/):

| Service | Status | Purpose |
|---|---|---|
| [Auth server](services/auth/) | Initial implementation | Self-hosted Go/PostgreSQL registration and Yggdrasil-compatible authentication for Minecraft communities |
| Mirror | Planned | Download mirror for versions, libraries, assets, and Java runtimes |
| Skin designer | Planned | Web skin editor and texture host |

Each service has its own Docker image and is optional; `umc` and `umcd` do not require them.

## Documentation

| User documentation | Developer documentation | Reference |
|---|---|---|
| [User guide](https://fps1024.github.io/UMC/en/guide/user-guide)<br />[Quickstart](https://fps1024.github.io/UMC/en/guide/quickstart)<br />[CLI reference](https://fps1024.github.io/UMC/en/guide/cli)<br />[Instances](https://fps1024.github.io/UMC/en/guide/instances)<br />[Java runtimes](https://fps1024.github.io/UMC/en/guide/java)<br />[Launching](https://fps1024.github.io/UMC/en/guide/launching)<br />[Troubleshooting](https://fps1024.github.io/UMC/en/guide/troubleshooting) | [Integration guide](https://fps1024.github.io/UMC/en/development/)<br />[Rust SDK](https://fps1024.github.io/UMC/en/development/sdk)<br />[Architecture](https://fps1024.github.io/UMC/en/development/architecture)<br />[Loader adapters](https://fps1024.github.io/UMC/en/development/loaders)<br />[JSON-RPC client guide](https://fps1024.github.io/UMC/en/development/rpc-client)<br />[Testing and CI](https://fps1024.github.io/UMC/en/development/testing)<br />[Contributing](https://fps1024.github.io/UMC/en/development/contributing) | [JSON-RPC v1](https://fps1024.github.io/UMC/en/reference/rpc-v1)<br />[Compatibility policy](https://fps1024.github.io/UMC/en/about/compatibility)<br />[Privacy Policy](https://fps1024.github.io/UMC/en/about/privacy)<br />[Terms of Service](https://fps1024.github.io/UMC/en/about/terms)<br />[v0.1.0 release notes](https://fps1024.github.io/UMC/en/reference/releases/v0.1.0) |

The documentation is available in [English](https://fps1024.github.io/UMC/en/) and
[简体中文](https://fps1024.github.io/UMC/zh/).

## Requirements

- Linux x86_64 GNU is the current release target.
- Rust 1.89 or newer when building from source.
- Network access for version metadata, game files, and Java runtimes.

Windows and macOS are planned ports and are not part of the validated release target yet.

## Repository layout

<details>
<summary>Show the repository structure</summary>

```text
crates/
  umc-api            public contracts
  umc-core           launcher engine
  umc-sdk            typed Rust facade
  umc-cli            umc CLI and umcd daemon
  umc-loader-api     loader adapter contract
  umc-loader-fabric  Fabric Meta adapter
  umc-loader-forge   Forge Maven adapter
  umc-loader-neoforge NeoForge Maven adapter
  umc-loader-registry built-in and host adapters
docs/
  en/                English documentation source
  zh/                Chinese translations
protocols/           canonical JSON-RPC documentation
scripts/             documentation sync and i18n helpers
```

</details>

## Community

Join the [UMC Discord server](https://discord.gg/d5qKvMp9Wc) to follow development, ask questions,
and share integrations.

- [Report an issue](https://github.com/FPS1024/UMC/issues)
- [Contributing guide](https://fps1024.github.io/UMC/en/development/contributing)

## License

UMC is licensed under the [MIT License](LICENSE).

UMC is an independent project. It is not an official Minecraft product and is not approved by or
associated with Mojang or Microsoft. Minecraft game files, Mojang/Microsoft marks, mod loaders, and
mods remain the property of their respective owners.
