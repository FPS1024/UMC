# UMC Auth

Optional self-hosted account and authentication service for Minecraft: Java Edition communities. It
provides a registration website and a Yggdrasil-compatible API for servers that use
[authlib-injector](https://github.com/yushijinhun/authlib-injector) or a launcher with external
authentication support.

This service is independent from the UMC launcher engine. A community can run it next to UMC or use
it with another launcher. It is written in Go and stores accounts in PostgreSQL.

## What it provides

- Account registration with username and password
- A stable 32-character Minecraft UUID per account
- Argon2id password hashing
- Opaque access tokens stored as SHA-256 hashes
- Yggdrasil `authenticate`, `refresh`, `validate`, `invalidate`, `signout`, and session `join` /
  `hasJoined` endpoints
- `GET /api/profiles/minecraft` and profile lookup endpoints
- Web sessions with HttpOnly cookies and CSRF protection
- HTTPS through Caddy with automatic certificates
- PostgreSQL storage with a persistent Docker volume
- A persistent RSA signing key for future profile-property signatures

## Requirements

- A Linux server with Docker and Docker Compose
- A domain pointing at the server
- TCP ports 80 and 443 reachable, plus UDP 443 for HTTP/3
- Outbound access for Caddy's certificate issuance

The Compose stack starts PostgreSQL 17 automatically. For local development outside Docker you need
Go 1.24+ and a PostgreSQL 17 database.

## Quick start

```sh
./scripts/setup.sh
# Edit AUTH_DOMAIN, POSTGRES_PASSWORD, SERVER_NAME, and BOOTSTRAP_TOKEN in .env
docker compose up -d --build
```

Caddy obtains a certificate automatically. Open:

```text
https://auth.example.com/
```

Register the first account at `/register`. If `BOOTSTRAP_TOKEN` is set, enter it on the registration
form. The first account is marked as an administrator and bypasses the invite, CAPTCHA, and email
requirements so the instance can be bootstrapped. Remove `BOOTSTRAP_TOKEN` after setup.

The Yggdrasil API root is:

```text
https://auth.example.com/api/yggdrasil
```

Give that URL to authlib-injector or to a launcher that supports external authentication. For
example:

```text
-javaagent:authlib-injector.jar=https://auth.example.com/api/yggdrasil
```

For a Minecraft server, start it with the same agent and keep `online-mode=true`:

```sh
java -javaagent:authlib-injector.jar=https://auth.example.com/api/yggdrasil \
  -jar server.jar nogui
```

Players register on the website, then sign in through their launcher. The server verifies each
join through `hasJoined` and receives the account UUID.

## Admin UI

The first registered account is an administrator. Sign in and open:

```text
https://auth.example.com/admin
```

- **Invite codes** — generate batches, set uses and expiry, copy codes, revoke codes
- **Users** — inspect accounts and disable non-admin users
- **Audit log** — recent administrator and registration actions
- **Dashboard** — current registration, CAPTCHA, and email modes plus recent activity

Invite codes are shown once. Only a SHA-256 hash is stored, so a lost code must be replaced.
The admin UI uses the same HttpOnly session cookie and CSRF protection as the rest of the site.

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `AUTH_DOMAIN` | required | Public hostname used by Caddy |
| `POSTGRES_PASSWORD` | required | PostgreSQL password, also passed to the auth service |
| `PUBLIC_ORIGIN` | `http://localhost:8080` | Public origin; Compose sets it to `https://${AUTH_DOMAIN}` |
| `SERVER_NAME` | `UMC Auth` | Name shown in the UI and Yggdrasil metadata |
| `BOOTSTRAP_TOKEN` | unset | If set, the first account must provide this token |
| `REGISTRATION_MODE` | `invite` | `invite`, `open`, or `closed` |
| `EMAIL_VERIFICATION` | `off` | `off` or `required`; required accounts must verify an email |
| `CAPTCHA_MODE` | `none` | `none`, `pow`, or `turnstile` |
| `POW_DIFFICULTY` | `16` | Leading zero bits for proof-of-work (12–24) |
| `REGISTER_LIMIT_PER_HOUR` | `3` | Registrations per IPv4 address or IPv6 /64 per hour |
| `REGISTER_LIMIT_PER_DAY` | `10` | Registrations per IPv4 address or IPv6 /64 per day |
| `REGISTER_LIMIT_GLOBAL_PER_HOUR` | `100` | Global registration budget per hour |
| `LOGIN_LIMIT_PER_MINUTE` | `10` | Sign-in and Yggdrasil authentication attempts per address per minute |
| `POW_CHALLENGE_LIMIT_PER_MINUTE` | `30` | Proof-of-work challenges per IPv4 address or IPv6 /64 per minute |
| `PROFILE_LOOKUP_LIMIT_PER_MINUTE` | `60` | Bulk profile lookups per IPv4 address or IPv6 /64 per minute |
| `TURNSTILE_SITE_KEY` / `TURNSTILE_SECRET_KEY` | unset | Required for `CAPTCHA_MODE=turnstile`; browsers must reach Cloudflare |
| `SMTP_HOST` / `SMTP_PORT` | unset | SMTP server for verification email |
| `SMTP_USERNAME` / `SMTP_PASSWORD` | unset | SMTP authentication when required |
| `SMTP_FROM` | unset | Sender address, for example `UMC Auth <no-reply@example.com>` |
| `SMTP_TLS` | `starttls` | `starttls`, `implicit`, or `none` |
| `ALLOW_INSECURE_HTTP` | `false` | Allow an `http://` PUBLIC_ORIGIN for local development only |
| `COOKIE_SECURE` | true for HTTPS | Mark session cookies as `Secure` |
| `SESSION_DAYS` | `30` | Web session lifetime |
| `TOKEN_DAYS` | `30` | Yggdrasil access-token lifetime |
| `DATABASE_URL` | local PostgreSQL DSN | PostgreSQL connection string |
| `PGPASSWORD` | from Compose | PostgreSQL password when not embedded in `DATABASE_URL` |
| `BIND_ADDR` | `0.0.0.0:8080` | Listen address inside the container |
| `TRUST_PROXY_HEADERS` | `false` | Trust `X-Forwarded-For` (Compose sets `true`) |
| `LOG_LEVEL` | `info` | `debug`, `info`, `warn`, or `error` |

Compose passes a keyword DSN and `PGPASSWORD`, so the password does not need URL escaping.

## Security notes

- **CAPTCHA choices:** `pow` is self-hosted and works without third-party connectivity.
  `turnstile` is usually invisible but depends on the browser reaching Cloudflare, which may be
  unreliable on some networks. Turnstile tokens are browser-only; API clients should use an invite
  code or `pow`.
- **HTTPS is required.** Caddy terminates TLS, redirects HTTP, and sets HSTS. Do not publish the
  `auth` container port directly. The service refuses to start with an `http://` PUBLIC_ORIGIN
  unless `ALLOW_INSECURE_HTTP=true` is set for local development.
- Passwords are hashed with Argon2id; access tokens and web session tokens are stored only as
  SHA-256 hashes.
- Session and CSRF cookies are HttpOnly and SameSite=Lax. CSRF tokens protect registration, login,
  and logout forms.
- Responses include `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`,
  `Permissions-Policy`, and a restrictive Content-Security-Policy.
- The auth container runs as UID 10001 with `no-new-privileges` and a read-only root filesystem.
  Only PostgreSQL has persistent storage.
- Back up the `auth-db` volume. It contains password hashes, token hashes, and the RSA private key
  used for future profile-property signatures; treat it as sensitive.

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
```

Website and admin:

```text
GET  /register
GET  /login
GET  /dashboard
GET  /verify-email?token=...
GET  /admin
GET  /admin/invites
GET  /admin/users
GET  /admin/audit
GET  /api/register/challenge
```

The API is intended to be reached over HTTPS. Health check:

```text
GET /healthz
```

## Data and backup

Accounts live in the `auth-db` PostgreSQL volume. Back up with `pg_dump`:

```sh
docker compose stop auth
docker compose exec -T db pg_dump -U umc_auth -d umc_auth -Fc > umc-auth.dump
docker compose start auth
```

Restore:

```sh
docker compose stop auth
docker compose exec -T db pg_restore -U umc_auth -d umc_auth --clean --if-exists < umc-auth.dump
docker compose start auth
```

## Development

Start a development database:

```sh
docker run --rm -d --name umc-auth-db \
  -e POSTGRES_DB=umc_auth \
  -e POSTGRES_USER=umc_auth \
  -e POSTGRES_PASSWORD=devpassword \
  -p 5432:5432 postgres:17-alpine
```

Run the service with Go 1.24+:

```sh
PUBLIC_ORIGIN=http://127.0.0.1:8080 \
ALLOW_INSECURE_HTTP=true \
COOKIE_SECURE=false \
BIND_ADDR=127.0.0.1:8080 \
DATABASE_URL="host=127.0.0.1 port=5432 user=umc_auth dbname=umc_auth sslmode=disable" \
PGPASSWORD=devpassword \
go run ./cmd/umc-auth
```

Tests:

```sh
./scripts/check.sh
```

Deployment check (requires the public domain and ports 80/443):

```sh
AUTH_DOMAIN=auth.example.com ./scripts/check-deployment.sh
```

It verifies HTTP-to-HTTPS redirects, certificate trust and expiry, HSTS, CSP, the health endpoint,
JSON metadata, the Secure cookie flag, and that the auth port is not exposed directly.

The PostgreSQL integration test runs when `TEST_DATABASE_URL` is set:

```sh
TEST_DATABASE_URL="host=127.0.0.1 port=5432 user=umc_auth dbname=umc_auth sslmode=disable" \
PGPASSWORD=devpassword go test -run TestStoreIntegration ./...
```

## Current limitations

- Email verification is optional, but password reset and account recovery are not implemented.
- No skin upload or texture signing; profiles expose an empty `properties` array.
- The admin UI covers invites, users, and audit logs; it does not include email resend or password
  reset.
- Schema creation uses idempotent `CREATE TABLE IF NOT EXISTS` statements; a dedicated migration
  system is not included yet.
- Rate limiting is in-process per auth container. If you run multiple replicas, add a shared limiter
  or edge proxy.
- No email resend endpoint; an expired verification link cannot be regenerated through the UI.
- Caddy expects ports 80/443. If another reverse proxy terminates TLS, run the `auth` service
  directly, set `COOKIE_SECURE=true`, and forward `X-Forwarded-For` and `X-Forwarded-Proto`.
