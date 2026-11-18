# Rust SDK

The UMC Rust SDK provides a native interface for applications integrating with UMC Core. SDK calls delegate to the same Core services used by the standalone CLI and daemon.

## Installation

The SDK is not published to crates.io yet. Use a path dependency while developing against a local checkout:

```toml
[dependencies]
umc-sdk = { path = "path/to/UMC/crates/umc-sdk" }
```

## Example

```rust
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

Use `Launcher::builder().connect_daemon()` to send calls to an existing local `umcd` process. See the [developer guide](./index.md) for integration boundaries and the [RPC v1 reference](../reference/rpc-v1.md) for operation details.

## Documentation status

This page covers initial setup only. Detailed reference pages for `Launcher`, instances, tasks, processes, and provider registration will be added as the public Rust API stabilizes.
