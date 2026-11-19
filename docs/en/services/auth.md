# UMC Auth

UMC Auth is an optional self-hosted account and authentication service for Minecraft: Java Edition
communities. It provides a registration website and a Yggdrasil-compatible API for servers that use
[authlib-injector](https://github.com/yushijinhun/authlib-injector) or a launcher with external
authentication support.

It is written in Go and stores accounts in PostgreSQL. The service is independent from `umc` and
`umcd`; a community can run it next to UMC or with another launcher.

## What it provides

- Username and password registration
- A stable 32-character Minecraft UUID per account
- Argon2id password hashing
- Yggdrasil `authenticate`, `refresh`, `validate`, `invalidate`, `signout`, `join`, `hasJoined`, and
  profile endpoints
- Web sessions with HttpOnly cookies and CSRF protection
- Admin UI for invite codes, users, and audit logs
- Optional email verification through SMTP
- Optional Cloudflare Turnstile or self-hosted proof-of-work
- Per-IP, IPv6 `/64`, and global rate limits
- HTTPS through Caddy with automatic certificates

## Requirements

- Linux server with Docker and Docker Compose
- A domain pointing at the server
- TCP ports 80 and 443 reachable, plus UDP 443 for HTTP/3
- Outbound access for Caddy's certificate issuance

The Compose stack starts PostgreSQL 17 automatically. Running outside Docker needs Go 1.24+ and a
PostgreSQL 17 database.

## Deploy

From the repository:

```sh
cd services/auth
./scripts/setup.sh
```

Edit `.env`:

```ini
AUTH_DOMAIN=auth.example.com
POSTGRES_PASSWORD=use-a-long-random-password
SERVER_NAME=Example Auth
BOOTSTRAP_TOKEN=use-a-long-random-bootstrap-token
REGISTRATION_MODE=invite
```

Start the stack:

```sh
docker compose up -d --build
```

Caddy obtains a certificate automatically. Open:

```text
https://auth.example.com/
```

Register the first account at `/register`. If `BOOTSTRAP_TOKEN` is set, enter it on the form. The
first account becomes an administrator and bypasses invites, CAPTCHA, and email verification so the
instance can be bootstrapped. Remove or change `BOOTSTRAP_TOKEN` after setup.

## Authentication endpoint

Give this URL to authlib-injector or a launcher that supports external authentication:

```text
https://auth.example.com/api/yggdrasil
```

For a Minecraft server, start it with the same agent and keep `online-mode=true`:

```sh
java -javaagent:authlib-injector.jar=https://auth.example.com/api/yggdrasil \
  -jar server.jar nogui
```

Players register on the website, then sign in through their launcher. The server verifies each join
through `hasJoined` and receives the account UUID.

## Registration modes

| Mode | Behavior |
|---|---|
| `invite` | An invite code is required. Recommended for private communities. |
| `open` | Anyone may register; combine with PoW or Turnstile and rate limits. |
| `closed` | New registrations are disabled. The first account can still bootstrap the instance. |

Set `REGISTRATION_MODE` in `.env`.

### Invite codes

Administrators create codes in:

```text
https://auth.example.com/admin/invites
```

A code can have a maximum number of uses, an expiry date, and a note. The plaintext code is shown
once; the database stores only its SHA-256 hash. Codes can be revoked at any time.

A registration link can prefill the code:

```text
https://auth.example.com/register?invite=UMC-XXXX-XXXX-XXXX
```

## Admin UI

Sign in with the first account and open `/admin`.

| Page | Purpose |
|---|---|
| `/admin` | User count, current modes, recent activity |
| `/admin/invites` | Create, inspect, and revoke invite codes |
| `/admin/users` | Inspect accounts and disable non-admin users |
| `/admin/audit` | Recent administrator and registration actions |

The admin UI uses the same HttpOnly session cookie and CSRF protection as the rest of the site.
There is no separate admin password and no two-factor authentication yet.

## Email verification

Email verification is optional because small self-hosted servers often have no reliable SMTP
service.

```ini
EMAIL_VERIFICATION=required
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USERNAME=...
SMTP_PASSWORD=...
SMTP_FROM=UMC Auth <no-reply@auth.example.com>
SMTP_TLS=starttls
```

`SMTP_TLS` accepts `starttls`, `implicit`, or `none`.

When enabled:

- New accounts are created unverified.
- A 24-hour single-use verification link is sent by email.
- Unverified accounts cannot sign in or use Yggdrasil authentication.
- Verification tokens are stored as hashes.

The first administrator is created verified so the instance can be initialized before SMTP is
working.

## Turnstile and proof-of-work

```ini
CAPTCHA_MODE=turnstile
TURNSTILE_SITE_KEY=...
TURNSTILE_SECRET_KEY=...
```

or:

```ini
CAPTCHA_MODE=pow
POW_DIFFICULTY=16
```

- **Turnstile** is usually invisible, but browsers must be able to reach Cloudflare. It may be
  unreliable on some networks. Tokens are browser-only and cannot be used by API clients.
- **Proof-of-work** is self-hosted. The browser solves a short SHA-256 challenge before
  registration. It works without third-party connectivity and is suitable for API clients.

The challenge is bound to the client address, expires after five minutes, and can be used once.

## Rate limits

| Variable | Default | Meaning |
|---|---|---|
| `REGISTER_LIMIT_PER_HOUR` | `3` | Registrations per IPv4 address or IPv6 `/64` per hour |
| `REGISTER_LIMIT_PER_DAY` | `10` | Registrations per address per day |
| `REGISTER_LIMIT_GLOBAL_PER_HOUR` | `100` | Global registration budget |
| `LOGIN_LIMIT_PER_MINUTE` | `10` | Sign-in and Yggdrasil authentication attempts |
| `POW_CHALLENGE_LIMIT_PER_MINUTE` | `30` | Proof-of-work challenge requests |
| `PROFILE_LOOKUP_LIMIT_PER_MINUTE` | `60` | Bulk profile lookups |

Limits are in-process and reset when the auth container restarts. For multiple replicas, add a
shared limiter or an edge proxy.

## HTTPS

The service refuses to start with an `http://` `PUBLIC_ORIGIN` unless:

```ini
ALLOW_INSECURE_HTTP=true
```

Use that only for local development. In production, Caddy terminates TLS, redirects HTTP to HTTPS,
sets HSTS, and keeps the auth container port private.

Verify a deployment from a machine with DNS access:

```sh
AUTH_DOMAIN=auth.example.com ./scripts/check-deployment.sh
```

The script checks:

- HTTP-to-HTTPS redirect
- Trusted TLS certificate chain
- Certificate validity for at least seven more days
- HSTS and Content-Security-Policy headers
- `/healthz`
- Yggdrasil metadata content type
- `Secure` on the CSRF cookie
- Port 8080 is not directly reachable

## API surface

Metadata:

```text
GET /api/yggdrasil
```

Yggdrasil:

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

Website:

```text
GET  /register
GET  /login
GET  /dashboard
GET  /verify-email?token=...
GET  /admin
GET  /healthz
```

## Backup and restore

Accounts and the RSA signing key live in the `auth-db` PostgreSQL volume.

```sh
docker compose stop auth
docker compose exec -T db pg_dump -U umc_auth -d umc_auth -Fc > umc-auth.dump
docker compose start auth
```

Restore while the auth service is stopped:

```sh
docker compose stop auth
docker compose exec -T db pg_restore -U umc_auth -d umc_auth --clean --if-exists < umc-auth.dump
docker compose start auth
```

Treat the database dump as sensitive: it contains password hashes, token hashes, and the private
signing key.

## Security notes

- Passwords use Argon2id; access tokens and web session tokens are stored only as SHA-256 hashes.
- Cookies are HttpOnly and SameSite=Lax; forms use CSRF tokens.
- The auth container runs as a non-root user with a read-only root filesystem.
- The PostgreSQL and Caddy ports are the only services exposed publicly; the auth application port
  stays internal.
- Public registration should normally use invites or a CAPTCHA/PoW mode plus rate limits.

## Current limitations

- No password reset or account recovery.
- No email resend endpoint; an expired verification link requires administrator help.
- No skin upload or texture signing yet; profiles expose an empty `properties` array.
- Admin settings are environment variables and require a restart.
- No two-factor authentication for administrators.
- The mirror is planned.

## Development

Run checks:

```sh
cd services/auth
./scripts/check.sh
```

Run locally against PostgreSQL:

```sh
PUBLIC_ORIGIN=http://127.0.0.1:8080 \
ALLOW_INSECURE_HTTP=true \
COOKIE_SECURE=false \
BIND_ADDR=127.0.0.1:8080 \
DATABASE_URL="host=127.0.0.1 port=5432 user=umc_auth dbname=umc_auth sslmode=disable" \
PGPASSWORD=devpassword \
go run ./cmd/umc-auth
```
