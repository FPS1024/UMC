---
layout: home

hero:
  text: 一个核心，驱动所有启动器
  tagline: "用 Rust 编写的无界面 Minecraft: Java Edition 启动引擎。安装、校验、启动并托管游戏进程，可通过 Rust SDK、本地 JSON-RPC 守护进程或 CLI 接入。"
  actions:
    - theme: brand
      text: 下载 Linux 版
      link: https://github.com/FPS1024/UMC/releases/latest
    - theme: alt
      text: 阅读文档
      link: /zh/guide/quickstart
    - theme: alt
      text: GitHub
      link: https://github.com/FPS1024/UMC

features:
  - icon:
      src: /icons/core.svg
      alt: 核心
    title: Rust 原生核心
    details: 同一个 umc-core 为 SDK、CLI 和守护进程提供能力。类型化契约、结构化错误，没有重复的启动逻辑。
  - icon:
      src: /icons/shield.svg
      alt: 完整性
    title: 完整性校验
    details: 客户端、库、资源、日志配置和运行时文件都会校验 SHA-1 与大小；失败的文件会重试，不会直接信任。
  - icon:
      src: /icons/tasks.svg
      alt: 任务
    title: 持久化任务
    details: 安装以持久化任务运行，支持带序号的进度事件、取消、重试、超时，以及守护进程重启后的恢复。
  - icon:
      src: /icons/lock.svg
      alt: 本地优先
    title: 本地优先
    details: 守护进程通过私有 Unix Socket 或命名管道提供 JSON-RPC。没有 TCP 监听、没有账号服务，也没有遥测。
  - icon:
      src: /icons/java.svg
      alt: Java
    title: 托管 Java
    details: 在启动前检测兼容的系统 Java，或按需下载 Mojang 运行时，并对每个 Minecraft 版本做兼容性检查。
  - icon:
      src: /icons/puzzle.svg
      alt: 加载器
    title: 加载器适配器
    details: Fabric、Forge 和 NeoForge 适配器可以查询版本并生成安装预检计划；执行安装与启动是下一阶段目标。
---
