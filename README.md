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
- [User guide](docs/guide/user-guide.md) — installation, building, first run, data locations, and common commands.
- [Developer and integration guide](docs/development/index.md) — Rust SDK, JSON-RPC, and GUI integration.
- [Architecture](docs/development/architecture.md)
- [RPC v1 protocol](protocols/rpc-v1.md)
- [Project, trademark, and privacy notes](docs/reference/legal-and-privacy.md)
- [Release and compatibility policy](docs/reference/release.md)

### Preview the documentation site

The documentation site is built with VitePress. Install Node.js 18 or newer, then run:

```sh
npm install
npm run docs:dev
```

The development server prints a local URL. `npm run docs:build` produces the static site in
`docs/.vitepress/dist/`.

UMC is licensed under MIT. It is an independent project, not an official Minecraft product, and is not approved by or associated with Mojang or Microsoft. See the [project and trademark notes](docs/reference/legal-and-privacy.md) for ownership boundaries and the current privacy summary.
