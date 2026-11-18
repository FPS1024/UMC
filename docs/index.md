# Universal Minecraft Core

**One Core. Every Launcher.**

UMC is a headless Minecraft: Java Edition launcher engine. It provides a shared Rust foundation that community applications can use as a library or operate as a standalone program through JSON-RPC.

## Project overview

UMC separates launcher functionality from user interfaces. Applications can integrate through:

- The Rust SDK
- The command-line interface
- JSON-RPC 2.0 over stdio or local daemon IPC

## Documentation

<div class="grid cards" markdown>

-   **Getting started**

    ---

    Build or install UMC, set up its data directory, and run a vanilla instance.

    [Read the user guide](user-guide.md)

-   **Architecture**

    ---

    Learn how the Core, SDK, CLI, RPC, and provider adapters fit together.

    [Read the architecture overview](architecture.md)

-   **Developer integration**

    ---

    Integrate UMC with a Rust application or a cross-language GUI.

    [Read the developer guide](developer-guide.md)

-   **RPC reference**

    ---

    Review the canonical JSON-RPC v1 methods, events, and limitations.

    [Read the RPC documentation](protocols/rpc-v1.md)

-   **Release policy**

    ---

    Check platform support, compatibility rules, and release procedures.

    [Read the release policy](release.md)

</div>

## Project status

UMC is under active development. Linux x86_64 is the current release platform. Vanilla installation and explicit offline launch are supported. Microsoft authentication is not implemented; Fabric, Forge, and NeoForge adapters currently provide version discovery and installation preflight only.

UMC is an independent community project, not an official Minecraft product and not approved by or associated with Mojang or Microsoft. Read the [project, trademark, and privacy notes](legal-and-privacy.md).

## Project links

- [GitHub repository](https://github.com/FPS1024/UMC)
- [Issue tracker](https://github.com/FPS1024/UMC/issues)
- [Releases](https://github.com/FPS1024/UMC/releases)
- [Community Discord](https://discord.gg/d5qKvMp9Wc)
