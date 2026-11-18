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
      text: 阅读文档（英文）
      link: /guide/quickstart
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

## 中文版正在完善

中文文档正在逐步补充。当前的快速开始、开发指南和 RPC 参考仍以英文提供，功能、命令和行为在所有语言版本中保持一致。

::: tip 语言切换
右上角的 `EN / 中文` 可以随时切换中英文首页；英文文档站保持不变。
:::

## 现在可以做什么

- 在 Linux x86_64 上安装并离线启动原版 Minecraft: Java Edition
- 对客户端、库和资源执行 SHA-1 与大小校验
- 检测系统 Java，或安装 Mojang 托管运行时
- 通过 CLI、Rust SDK 或本地 JSON-RPC 守护进程接入
- 查看持久化任务进度、取消任务，并在守护进程重启后恢复进程状态

## 开发者资源

- [快速开始（英文）](/guide/quickstart)
- [集成指南（英文）](/development/)
- [JSON-RPC v1 参考（英文）](/reference/rpc-v1)
- [GitHub 仓库](https://github.com/FPS1024/UMC)

> UMC 是独立社区项目，不是官方 Minecraft 产品，也与 Mojang 或 Microsoft 没有关联。
