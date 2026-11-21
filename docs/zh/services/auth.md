---
source: en/services/auth.md
---

# UMC Auth

UMC Auth 是一个可选的、自托管的 Minecraft: Java Edition 社区账号与认证服务。它提供注册网站，以及供 [authlib-injector](https://github.com/yushijinhun/authlib-injector) 或支持外置登录的启动器使用的 Yggdrasil 兼容 API。

它使用 Go 编写，账号存储在 PostgreSQL 中。该服务独立于 `umc` 和 `umcd`，社区可以把它和 UMC 配套运行，也可以配其他启动器使用。

## 提供的能力

- 用户名和密码注册
- 每个账号一个稳定的 32 位 Minecraft UUID
- Argon2id 密码哈希
- Yggdrasil `authenticate`、`refresh`、`validate`、`invalidate`、`signout`、`join`、`hasJoined` 和档案接口
- 带 HttpOnly Cookie 和 CSRF 保护的 Web 会话
- 管理邀请码、用户和审计日志的后台
- 可选的 SMTP 邮箱验证
- 可选的 Cloudflare Turnstile 或自建工作量证明
- 按 IP、IPv6 `/64` 和全局限流
- 通过 Caddy 自动获得证书的 HTTPS

## 环境要求

- 带 Docker 和 Docker Compose 的 Linux 服务器
- 指向服务器的域名
- TCP 80/443 可访问，HTTP/3 需要 UDP 443
- Caddy 申请证书所需的出站网络

Compose 会自动启动 PostgreSQL 17。如果在 Docker 外运行，需要 Go 1.24+ 和 PostgreSQL 17。

## 部署

在仓库中执行：

```sh
cd services/auth
./scripts/setup.sh
```

编辑 `.env`：

```ini
AUTH_DOMAIN=auth.example.com
POSTGRES_PASSWORD=使用一个长随机密码
SERVER_NAME=Example Auth
BOOTSTRAP_TOKEN=使用一个长随机引导令牌
REGISTRATION_MODE=invite
```

启动：

```sh
docker compose up -d --build
```

Caddy 会自动申请证书。打开：

```text
https://auth.example.com/
```

在 `/register` 注册第一个账号。如果设置了 `BOOTSTRAP_TOKEN`，注册页会要求填写。第一个账号会成为管理员，并绕过邀请码、CAPTCHA 和邮箱验证，以便初始化实例。初始化完成后请删除或更换 `BOOTSTRAP_TOKEN`。

## 认证地址

把下面的地址填入 authlib-injector 或支持外置登录的启动器：

```text
https://auth.example.com/api/yggdrasil
```

Minecraft 服务端使用同样的 agent，并保持 `online-mode=true`：

```sh
java -javaagent:authlib-injector.jar=https://auth.example.com/api/yggdrasil \
  -jar server.jar nogui
```

玩家先在网站注册，然后通过启动器登录。服务端通过 `hasJoined` 验证每次进入，并拿到账号 UUID。

## 注册模式

| 模式 | 行为 |
|---|---|
| `invite` | 必须提供邀请码，适合私有社区 |
| `open` | 所有人可注册，应配合 PoW 或 Turnstile 以及限流 |
| `closed` | 关闭新注册，但第一个账号仍可用于初始化 |

在 `.env` 中设置 `REGISTRATION_MODE`。

### 邀请码

管理员在下面的页面创建邀请码：

```text
https://auth.example.com/admin/invites
```

邀请码可以设置最大使用次数、过期时间和备注。明文只在创建时显示一次，数据库只保存 SHA-256 哈希，并且可以随时吊销。

注册链接可以预填邀请码：

```text
https://auth.example.com/register?invite=UMC-XXXX-XXXX-XXXX
```

## 管理后台

用第一个账号登录，然后打开 `/admin`。

| 页面 | 用途 |
|---|---|
| `/admin` | 用户数量、当前模式、最近操作 |
| `/admin/invites` | 创建、查看和吊销邀请码 |
| `/admin/users` | 查看账号并禁用非管理员用户 |
| `/admin/audit` | 最近的管理和注册操作 |

管理后台和普通页面使用同一套 HttpOnly 会话 Cookie 和 CSRF 保护。目前没有独立的管理员密码，也没有两步验证。

## 邮箱验证

小规模自建服务器不一定有稳定的 SMTP 服务，所以邮箱验证是可选的。

```ini
EMAIL_VERIFICATION=required
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USERNAME=...
SMTP_PASSWORD=...
SMTP_FROM=UMC Auth <no-reply@auth.example.com>
SMTP_TLS=starttls
```

`SMTP_TLS` 支持 `starttls`、`implicit` 和 `none`。

启用后：

- 新账号初始为未验证状态。
- 发送一封 24 小时有效的一次性验证链接。
- 未验证账号不能登录，也不能通过 Yggdrasil 认证。
- 验证 Token 只保存哈希。

第一个管理员会自动视为已验证，以便在 SMTP 尚未配置好时完成初始化。

## Turnstile 与工作量证明

```ini
CAPTCHA_MODE=turnstile
TURNSTILE_SITE_KEY=...
TURNSTILE_SECRET_KEY=...
```

或者：

```ini
CAPTCHA_MODE=pow
POW_DIFFICULTY=16
```

- **Turnstile** 通常无感，但浏览器必须能访问 Cloudflare，部分网络下可能不稳定。它的 Token 只适用于浏览器，不能给 API 客户端使用。
- **工作量证明** 完全自建。浏览器在注册前求解一个短 SHA-256 挑战，不依赖第三方网络，也适合 API 客户端。

挑战绑定客户端地址，5 分钟后过期，并且只能使用一次。

## 限流

| 变量 | 默认值 | 含义 |
|---|---|---|
| `REGISTER_LIMIT_PER_HOUR` | `3` | 每个 IPv4 地址或 IPv6 `/64` 每小时注册数 |
| `REGISTER_LIMIT_PER_DAY` | `10` | 每个地址每天注册数 |
| `REGISTER_LIMIT_GLOBAL_PER_HOUR` | `100` | 全站每小时注册预算 |
| `LOGIN_LIMIT_PER_MINUTE` | `10` | 登录和 Yggdrasil 认证尝试次数 |
| `POW_CHALLENGE_LIMIT_PER_MINUTE` | `30` | 工作量证明挑战请求数 |
| `PROFILE_LOOKUP_LIMIT_PER_MINUTE` | `60` | 批量用户名查询次数 |

限流在 auth 容器内存中实现，容器重启后会重置。多副本部署时请增加共享限流或边缘代理。

## HTTPS

当 `PUBLIC_ORIGIN` 是 `http://` 时，服务默认拒绝启动，除非设置：

```ini
ALLOW_INSECURE_HTTP=true
```

这只应在本机开发时使用。生产环境中 Caddy 负责 TLS、HTTP 到 HTTPS 跳转、HSTS，并保证 auth 容器端口不直接对外。

在能解析 DNS 的机器上验证部署：

```sh
AUTH_DOMAIN=auth.example.com ./scripts/check-deployment.sh
```

脚本会检查：

- HTTP 是否跳转 HTTPS
- TLS 证书链是否可信
- 证书是否至少还有 7 天
- HSTS 和 Content-Security-Policy
- `/healthz`
- Yggdrasil 元数据 Content-Type
- CSRF Cookie 是否带 `Secure`
- 8080 端口是否未直接暴露

## API

元数据：

```text
GET /api/yggdrasil
```

Yggdrasil：

```text
POST /api/yggdrasil/authserver/authenticate
POST /api/yggdrasil/authserver/refresh
POST /api/yggdrasil/authserver/validate
POST /api/yggdrasil/authserver/invalidate
POST /api/yggdrasil/authserver/signout
POST /api/yggdrasil/sessionserver/session/minecraft/join
GET  /api/yggdrasil/sessionserver/session/minecraft/hasJoined
GET  /api/yggdrasil/sessionserver/session/minecraft/profile/{uuid}
GET  /api/yggdrasil/api/user/profile/{uuid}
POST /api/yggdrasil/api/profiles/minecraft
GET  /api/register/challenge
```

网站：

```text
GET  /register
GET  /login
GET  /dashboard
GET  /verify-email?token=...
GET  /admin
GET  /healthz
```

## 备份与恢复

账号和 RSA 签名密钥保存在 `auth-db` PostgreSQL 卷中。

```sh
docker compose stop auth
docker compose exec -T db pg_dump -U umc_auth -d umc_auth -Fc > umc-auth.dump
docker compose start auth
```

恢复时先停止 auth 服务：

```sh
docker compose stop auth
docker compose exec -T db pg_restore -U umc_auth -d umc_auth --clean --if-exists < umc-auth.dump
docker compose start auth
```

数据库转储包含密码哈希、Token 哈希和私钥，请按敏感数据保管。

## 安全说明

- 密码使用 Argon2id；访问 Token 和 Web 会话 Token 只保存 SHA-256 哈希。
- Cookie 为 HttpOnly + SameSite=Lax；表单使用 CSRF Token。
- auth 容器以非 root 用户运行，根文件系统只读。
- 公网只暴露 PostgreSQL 和 Caddy 所需端口；auth 应用端口保持内部访问。
- 公开注册通常应使用邀请码，或启用 CAPTCHA/PoW 并配合限流。

## 当前限制

- 没有找回密码或账号恢复。
- 没有验证邮件重发；链接过期后需要管理员协助。
- 尚未实现皮肤上传和纹理签名，档案的 `properties` 为空。
- 管理员设置仍是环境变量，修改后需要重启。
- 管理员没有两步验证。
- 镜像站已有首版 Compose 部署，但尚未接入 UMC Core。

## 开发

运行检查：

```sh
cd services/auth
./scripts/check.sh
```

本机连接 PostgreSQL 运行：

```sh
PUBLIC_ORIGIN=http://127.0.0.1:8080 \
ALLOW_INSECURE_HTTP=true \
COOKIE_SECURE=false \
BIND_ADDR=127.0.0.1:8080 \
DATABASE_URL="host=127.0.0.1 port=5432 user=umc_auth dbname=umc_auth sslmode=disable" \
PGPASSWORD=devpassword \
go run ./cmd/umc-auth
```
