# UMC Mirror

UMC Mirror is an optional Go service that fetches immutable Minecraft artifacts from configured
upstream providers, verifies each object against an expected SHA-1, and caches only verified bytes.
It is distributed as a Docker image and Compose stack. UMC Core does not use it automatically yet.

## Integrity and transport rules

- Every object request includes the expected 40-character SHA-1. The launcher should take this value
  from Mojang metadata obtained over HTTPS. Objects without an expected SHA-1 are not served by this
  API.
- The mirror hashes the full upstream response before it is stored or returned. A mismatch is
  discarded and returned as `502`; the object never enters the cache.
- Cached objects are hashed again before every cache hit is served.
- Every configured provider `base_url` must use HTTPS. Plain HTTP, credentials, query strings, and
  fragments are rejected at startup. Redirects must remain HTTPS and target another configured host.
- The public URL must also be HTTPS. The Compose stack puts Caddy in front of the service for
  automatic TLS. Do not publish the mirror's internal port `8080` directly to the Internet.
- The client does not use environment HTTP proxies, and upstream DNS addresses must be public
  unicast addresses. This prevents a configured provider or redirect from reaching local services.

The mirror verifies that bytes match the expected digest; the launcher is responsible for obtaining
that expected digest from Mojang's HTTPS metadata. SHA-1 is used because it is the integrity field
published by the Minecraft metadata format, not as a modern signature or proof of publisher identity.

## API

```text
GET /v1/objects/{provider}/{sha1}/{upstream-path}
HEAD /v1/objects/{provider}/{sha1}/{upstream-path}
GET /healthz
```

Example using a SHA-1 read from Mojang metadata:

```sh
curl --fail --location \
  "https://mirror.example.org/v1/objects/mojang-data/<sha1>/v1/objects/<sha1>/client.jar" \
  -o client.jar
```

Provider IDs and HTTPS origins are configured in `providers.json`. The request path is appended to
the configured origin; clients cannot select an arbitrary hostname. Objects are addressed in the
cache by SHA-1 and served with an immutable cache policy and an ETag.

## Docker Compose

```sh
cp .env.example .env
# Edit MIRROR_DOMAIN in .env to a DNS name pointed at this host.
docker compose up -d --build
```

Ports 80 and 443 must be reachable for Caddy's certificate provisioning and HTTPS service. The
cache is stored in the `mirror-cache` Docker volume. `MAX_OBJECT_BYTES` limits one download and
`CACHE_MAX_BYTES` limits total cache size. When the cache is full, the oldest objects are evicted.
`MAX_CONCURRENT_DOWNLOADS` bounds simultaneous upstream transfers.

## Current scope

- Go 1.24, Linux container, and Docker Compose deployment.
- HTTPS-only configured Mojang/launcher origins and redirect allowlisting.
- Content-addressed SHA-1 verification, atomic cache writes, per-object request serialization, and
  a health endpoint.
- No uploads, account system, mod hosting, rewriting, or automatic Core integration.
- No serving of metadata objects that lack a trusted expected SHA-1, metrics, or automated
  Windows/macOS deployment.

This is the first implementation and needs operational and integration review before broad public
deployment. UMC Mirror is independent from UMC Auth; it does not authenticate players or verify
Minecraft ownership.
