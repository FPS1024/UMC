---
source: en/reference/legal-and-privacy.md
---

# 项目、商标与隐私说明

本页说明当前 UMC 项目预期的所有权边界和软件行为。它不是法律意见。当 UMC 增加账号认证、遥测、托管服务、模组分发或其他重要数据流时，应更新本说明。

## 项目与 Minecraft 的关系

UMC（Universal Minecraft Core）是独立的社区软件项目。它不由 Mojang Studios 或 Microsoft 制作、批准、背书或支持，也不是官方 Minecraft 产品。“Minecraft”这一名称用于标识 UMC 所设计的游戏；它不代表所有权、合作关系或背书。

在项目列表、发布说明和应用 About/Legal 页面中使用清晰声明：

> UMC 是独立项目。它不是官方 Minecraft 产品，也未获得 Mojang 或 Microsoft 的批准或关联。

Mojang 和 Microsoft 保留对 Minecraft 名称、商标、徽标、游戏、启动器及相关材料的权利。UMC 集成不应以暗示官方地位的方式使用其徽标或呈现方式。分发或推广 Minecraft 相关产品前，请阅读当前的 [Minecraft Usage Guidelines](https://www.minecraft.net/en-us/usage-guidelines) 和 [Minecraft EULA](https://www.minecraft.net/en-us/eula)。

## 归属边界

| 材料 | 项目边界 |
|---|---|
| UMC 编写的源代码和文档 | 版权归列出的 UMC 作者/贡献者所有，并按本仓库的 MIT 许可证授权。 |
| UMC 名称和项目美术 | 用于标识这个独立项目。使用 UMC 品牌的许可不应被表述为使用 Minecraft 或 Mojang/Microsoft 品牌的许可。 |
| Minecraft 游戏文件、Minecraft/Mojang/Microsoft 商标及其在线服务 | 不归 UMC 所有或许可。UMC 不在发布包中捆绑游戏；当前引擎根据用户请求的安装，从元数据和下载源获取游戏文件。 |
| Microsoft、Xbox 和 Minecraft 账号凭据及配置文件信息 | 不是 UMC 财产。当前版本未实现 Microsoft 认证。未来支持必须说明交换和存储的内容，且不得收集用户的 Microsoft 密码。 |
| Fabric、Forge、NeoForge、模组平台、模组和整合包 | 由各自独立维护或创建。其名称、代码、内容、商标、条款和许可证仍归各自所有者。UMC 的适配器不授予这些项目或内容的权利。 |
| Rust 依赖 | 由各自作者声明的许可证管辖的第三方软件。发布打包应包含适用的声明；UMC 的 MIT 许可证不能替代依赖许可证。 |

MIT 适用于 UMC 自己的受覆盖软件。它不授予 Minecraft 文件、第三方服务、模组或商标的权利。“Powered by UMC Core”是可选的集成署名，不是背书，也不是 MIT 要求。再分发说明见 [开发者指南](../development/index.md)。

## 当前版本隐私摘要

- 当前版本中，UMC 不运营用户账号服务、云同步服务、遥测端点或远程管理服务。
- UMC 在 `UMC_DATA_DIR`（开发环境中为 `./umc-data`）下本地存储托管游戏文件、实例设置、任务/进程状态和日志。日志可能包含玩家名称、本地路径、游戏输出或模组输出。
- 版本查询、游戏安装、Java 运行时安装和加载器元数据查询会向上游服务直接发起 HTTPS 请求。这些服务可能收到普通连接元数据，包括客户端 IP 地址。
- 守护进程只监听本地 IPC 端点。访问数据目录及其 Socket/管道即获得本地控制权；用户应保持该目录私有。
- Microsoft 账号认证尚未实现。本版本不存储 Microsoft 密码、OAuth Token 或刷新 Token。

用户应在分享前检查日志，不应发布账号凭据或游戏/模组打印的私人服务器信息。如果集成 GUI 增加遥测、账号存储、崩溃报告或云功能，其发布者必须单独说明这些功能。

## 未来的账号支持

在发布 Microsoft 认证之前，UMC 应更新本说明和用户指南，说明实际注册的客户端身份、登录流程、服务端点、Token 存储机制、刷新/注销行为、账号删除步骤以及 UMC 保留的任何数据。Token 不得写入普通实例配置或日志。账号登录不会使 UMC 成为 Microsoft 或 Mojang 服务，用户仍需遵守相关服务的条款。

## 保证与支持边界

UMC 按 MIT 许可证提供，包括其“AS IS”保证免责和责任限制。Mojang/Microsoft 以及第三方模组或加载器维护者不会仅因 UMC 与其软件或服务交互而对 UMC 提供支持。UMC 特有问题请通过项目的 GitHub Issue 跟踪器报告；游戏、账号、加载器和模组问题可能需要联系各自的维护者。
