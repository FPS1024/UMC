# UMC — Universal Minecraft Core

**One Core. Every Launcher.** UMC is a headless Minecraft: Java Edition launcher engine written in Rust. It is designed to run both as a standalone program and as a library, with both entry points using the same Core implementation. Community applications can integrate through the Rust SDK or JSON-RPC.

## Project status

The current Linux x86_64 release supports vanilla version discovery and installation, Java compatibility checks, isolated instances, asynchronous tasks, and explicit offline launch. Microsoft authentication and authenticated online play are not implemented.

Fabric, Forge, and NeoForge adapters currently provide version discovery and installation preflight plans. They do not yet install or launch loader instances. Mod catalogs such as CurseForge and Modrinth are separate from loader installation.

Linux x86_64 is the active platform target. Windows and macOS are planned ports.

## Community

Join the [UMC Discord server](https://discord.gg/d5qKvMp9Wc) to follow development and connect with the community.

## Documentation

- [Official documentation website](https://fps1024.github.io/UMC/)
- [User guide](docs/user-guide.md) — installation, building, first run, data locations, and common commands.
- [Developer and integration guide](docs/developer-guide.md) — Rust SDK, JSON-RPC, and GUI integration.
- [Architecture](docs/architecture.md)
- [RPC v1 protocol](protocols/rpc-v1.md)
- [Project, trademark, and privacy notes](docs/legal-and-privacy.md)
- [Release and compatibility policy](docs/release.md)

UMC is licensed under MIT. It is an independent project, not an official Minecraft product, and is not approved by or associated with Mojang or Microsoft. See the [project and trademark notes](docs/legal-and-privacy.md) for ownership boundaries and the current privacy summary.
