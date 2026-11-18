# UMC — Universal Minecraft Core

**One Core. Every Launcher.** A headless Minecraft Java launcher engine, designed to ship both as a Rust library and as a standalone executable. Both entry points delegate to `umc-core`; adapters contain no launcher business logic.

## Current scope

The workspace provides the shared Core, a Rust SDK, CLI, JSON-RPC 2.0 over newline-delimited stdio, Mojang version metadata, SHA-1/size checked vanilla downloads, Java compatibility detection and optional Mojang runtime provisioning, persistent asynchronous install tasks, isolated instances, and offline launch/process control. Microsoft authentication is not implemented yet.

The loader adapter crates currently expose Fabric, Forge, and NeoForge version discovery and loader-specific installation preflight plans through RPC and the Rust SDK. Executing those plans, installing loader instances, and launching them are not supported yet. Mod content catalog APIs (for example CurseForge and Modrinth) remain separate.

UMC is an independent community project. It is not an official Minecraft product and is not approved by or associated with Mojang or Microsoft. See [project, trademark, and privacy notes](docs/legal-and-privacy.md) for ownership boundaries, network behavior, and account-data plans.

## Community

Join the [UMC Discord server](https://discord.gg/d5qKvMp9Wc) to follow development and connect with the community.

## Build and run

```sh
cargo build --workspace
cargo run -p umc -- version list
cargo run -p umc -- instance create Survival <minecraft-version>
cargo run -p umc -- instance list
cargo run -p umc -- instance repair <instance-id>
cargo run -p umc -- instance install <instance-id>
cargo run -p umc -- task get <task-id>
cargo run -p umc -- java detect 21
cargo run -p umc -- java runtimes
cargo run -p umc -- java install <minecraft-version>
cargo run -p umc -- process list
cargo run -p umc -- launch <instance-id> Player --offline
cargo run -p umc -- rpc --stdio
cargo run -p umc --bin umcd
```

See the [user guide](docs/user-guide.md) for first-run usage and local data details, and [installation instructions](docs/install.md) for Rust requirements, per-user installation, CI artifacts, and setup. Linux x86_64 is the active platform target; Windows and macOS are planned ports.

Version lookup, game installation, and explicit runtime installation need network access. Set `UMC_DATA_DIR` to choose the cache and managed game data directory (default: `./umc-data`). The stdio transport reads one JSON-RPC request per line and writes one response per line. Managed Java downloads are opt-in and stored below `runtimes/`; launch prefers a compatible managed runtime matching the release's Mojang component, then falls back to system Java. `--offline` is explicit; this does not sign in to Microsoft or enable authenticated online play. The CLI waits for the game to exit and saves its console output under the instance logs directory.

Install RPCs return a task ID immediately. Use `task.get`, `task.cancel`, and `task.events` to follow work; events are persisted with per-task sequence numbers. The Rust SDK exposes `install`, `install_java_runtime`, `task`, `cancel_task`, `task_events`, and `subscribe_task`. Task deadlines default to six hours; file downloads retry three times, run with a shared 32-download limit, and support cancellation. The one-shot CLI waits for install tasks and prints progress to stderr. `umc task get|cancel|events` connects to a running `umcd` using the configured data directory, so persistent daemon tasks can be inspected or cancelled from another process.

`umcd` exposes the same JSON-RPC methods over a local IPC endpoint. On Unix it uses `UMC_DATA_DIR/umc.sock` with mode `0600` and a private data directory; on Windows it uses the `\\.\pipe\umc` named pipe with an owner-only ACL and rejects remote clients. Rust clients can select this transport with `Launcher::builder().data_dir(...).connect_daemon()`. After a daemon restart, `process.list` reconciles saved PIDs against a per-launch identity marker. Stopped or missing processes are marked exited; their exit code is unknown if the old supervisor was gone.

Example:

```json
{"jsonrpc":"2.0","id":1,"method":"system.info","params":{}}
```

## Rust SDK

```rust,no_run
use umc_sdk::Launcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let launcher = Launcher::builder().data_dir("./umc-data").build().await?;
    for version in launcher.versions().await? {
        println!("{}", version.id);
    }
    Ok(())
}
```

UMC is licensed under MIT. See the [developer and integration guide](docs/developer-guide.md), [architecture](docs/architecture.md), [RPC v1](protocols/rpc-v1.md), [project, trademark, and privacy notes](docs/legal-and-privacy.md), [installation instructions](docs/install.md), and the [release policy](docs/release.md) for details.

