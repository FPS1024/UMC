# Developer and Integration Guide

UMC is designed so community applications can use one Core through either the Rust SDK or JSON-RPC. The CLI, SDK, and daemon delegate launcher operations to the same `umc-core` implementation.

## Choose an integration

- **Rust application:** use `umc-sdk` and call its typed methods. This runs Core in-process.
- **Cross-language GUI:** communicate with `umcd` through local IPC or run `umc rpc --stdio` as a child process using newline-delimited JSON-RPC 2.0.
- **Shell automation:** use the `umc` CLI for commands and scripts; use task methods for long-running operations.

See [RPC v1](../protocols/rpc-v1.md) for methods, event sequencing, task semantics, and compatibility policy. See [architecture](architecture.md) for crate boundaries.

## Rust SDK

During development, point Cargo at the SDK crate:

```toml
[dependencies]
umc-sdk = { path = "../UMC/crates/umc-sdk" }
```

```rust,no_run
use umc_sdk::Launcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let launcher = Launcher::builder()
        .data_dir("./umc-data")
        .build()
        .await?;

    for version in launcher.versions().await? {
        println!("{}", version.id);
    }
    Ok(())
}
```

The SDK crate is not yet published to crates.io. Rust hosts can register trusted, in-process loader adapters using `LauncherBuilder::register_loader`. This registers compiled Rust code; UMC does not download or dynamically load plugins. When using a separate daemon, adapters must be compiled and registered by the daemon host.

## JSON-RPC and long-running work

Use JSON-RPC method results and task events as the integration contract. Do not parse terminal progress output to discover task state. Install methods return task IDs; clients can query, cancel, and subscribe to sequenced events. Ignore unknown optional fields and event types to remain forward-compatible.

The stdio transport uses one JSON-RPC message per line. Daemon IPC is local to the configured data directory. Do not expose or proxy the daemon as a remote service without adding an explicit authenticated and authorized boundary.

## Attribution and redistribution

UMC is licensed under MIT. Preserve its copyright and license text when redistributing UMC or substantial portions of it. Include the license in application packages, such as in a `licenses/` folder or an About/Legal screen that provides the complete notice.

An application may optionally acknowledge the integration with “Powered by UMC Core” in its About page, credits, or documentation. This is a community attribution suggestion, not a requirement of the MIT license. Do not imply that UMC endorses the integrating application. The Minecraft non-affiliation notice is separate; see [Project, Trademark, and Privacy Notes](legal-and-privacy.md).

The integrating application remains responsible for documenting its own telemetry, network requests, account handling, and privacy practices. UMC's local privacy summary does not automatically cover behavior added by a GUI or another host application.
