#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

if [ ! -f .env ]; then
	cp .env.example .env
	echo "Created .env. Set AUTH_DOMAIN, POSTGRES_PASSWORD, and SERVER_NAME, then run:"
else
	echo ".env already exists. Review AUTH_DOMAIN, POSTGRES_PASSWORD, and SERVER_NAME, then run:"
fi

echo "  docker compose up -d --build"
