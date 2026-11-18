---
layout: home

hero:
  text: One Core. Every Launcher.
  tagline: "A headless Minecraft: Java Edition launcher engine written in Rust. Install, verify, launch, and supervise the game from your own application — through a Rust SDK, a local JSON-RPC daemon, or the CLI."
  actions:
    - theme: brand
      text: Download for Linux
      link: https://github.com/FPS1024/UMC/releases/latest
    - theme: alt
      text: Read the docs
      link: /guide/quickstart
    - theme: alt
      text: View on GitHub
      link: https://github.com/FPS1024/UMC

features:
  - icon:
      src: /icons/core.svg
      alt: Core
    title: Rust-native core
    details: One shared umc-core implementation powers the SDK, CLI, and daemon. Typed contracts, structured errors, and no duplicated launcher logic.
  - icon:
      src: /icons/shield.svg
      alt: Integrity
    title: Integrity checked
    details: Every client, library, asset, log config, and runtime file is verified by SHA-1 and size before it is used. Failed files are retried, never trusted.
  - icon:
      src: /icons/tasks.svg
      alt: Tasks
    title: Persistent tasks
    details: Installs run as durable tasks with sequenced progress events, cancellation, retries, timeouts, and recovery after a daemon restart.
  - icon:
      src: /icons/lock.svg
      alt: Local
    title: Local-first by default
    details: The daemon speaks JSON-RPC over a private Unix socket or named pipe. There is no TCP listener, no account service, and no telemetry.
  - icon:
      src: /icons/java.svg
      alt: Java
    title: Managed Java
    details: Detect compatible system Java or provision Mojang runtimes on demand, with per-version compatibility checks before launch.
  - icon:
      src: /icons/puzzle.svg
      alt: Loaders
    title: Loader-ready adapters
    details: Fabric, Forge, and NeoForge adapters discover versions and produce loader-specific install plans. Execution is the next milestone.
---
