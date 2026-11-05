#!/bin/sh
# Example development launcher for Arteries.
#
# Copy this file to runserv.sh (which is ignored) and edit the environment
# values, or run it directly with them set.
#
# Required:
#   DIAMOND_POSTGRES_URL   PostgreSQL connection string for the app database.
#                          The database must already exist. The script
#                          initializes it or applies the supported migration.
#   UI_PASSWORD            Password for the development account when it is
#                          created. Never commit a real password.
#
# Optional:
#   UI_USER                Account name (default: player)
#   MAILBOX                Private identity mail directory (default: .diamond/mailbox)
#   PORT                   Loopback HTTP port (default: 3032)
#   DIAMOND                Path to the Diamond executable (default: PATH lookup)
#   RUNNER                 Path to the arteries-runner executable (default: PATH lookup)
#   DIAMOND_MIGRATION_POSTGRES  Migration connection string (default:
#                          DIAMOND_POSTGRES_URL; use a migration-capable role)
#
# Example:
#   DIAMOND_POSTGRES_URL='host=/path/to/socket dbname=arteries user=me' \
#   UI_PASSWORD='a local development password' \
#   sh runserv.example.sh

set -eu

DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi

RUNNER=${RUNNER:-$(command -v arteries-runner || true)}
if [ -z "$RUNNER" ]; then
  echo "arteries-runner not found; build runner/ or set RUNNER=/path/to/arteries-runner" >&2
  exit 1
fi

: "${DIAMOND_POSTGRES_URL:?set DIAMOND_POSTGRES_URL to the app database connection string}"
: "${UI_PASSWORD:?set UI_PASSWORD for the development account}"

UI_USER=${UI_USER:-player}
MAILBOX=${MAILBOX:-.diamond/mailbox}
PORT=${PORT:-3032}

case "$UI_USER" in
  ''|*[!A-Za-z0-9_-]*)
    echo "UI_USER must contain only letters, digits, underscores, and hyphens" >&2
    exit 1
    ;;
esac

mkdir -p "$MAILBOX"
chmod 700 "$MAILBOX"

rm -rf build
"$DIAMOND" build app/diamond.toml --out build

# Initialize a fresh database, or apply the supported additive migrations when
# an existing database is behind the current schema version.
status=$("$DIAMOND" project-status build --postgres-env DIAMOND_POSTGRES_URL 2>/dev/null || true)
if [ -z "$status" ]; then
  "$DIAMOND" project-init build --postgres-env DIAMOND_POSTGRES_URL
else
  actual=$(printf '%s' "$status" | sed -n 's/.*"version":\([0-9][0-9]*\).*/\1/p')
  expected=$(printf '%s' "$status" | sed -n 's/.*"expected_version":\([0-9][0-9]*\).*/\1/p')
  if [ -z "$actual" ] || [ -z "$expected" ]; then
    echo "cannot read the database schema version" >&2
    exit 1
  fi
  while [ "$actual" != "$expected" ]; do
    migration=${DIAMOND_MIGRATION_POSTGRES:-$DIAMOND_POSTGRES_URL}
    DIAMOND_MIGRATION_POSTGRES=$migration \
      "$DIAMOND" project-migrate build --postgres-env DIAMOND_MIGRATION_POSTGRES
    status=$("$DIAMOND" project-status build --postgres-env DIAMOND_POSTGRES_URL 2>/dev/null || true)
    actual=$(printf '%s' "$status" | sed -n 's/.*"version":\([0-9][0-9]*\).*/\1/p')
    expected=$(printf '%s' "$status" | sed -n 's/.*"expected_version":\([0-9][0-9]*\).*/\1/p')
    if [ -z "$actual" ] || [ -z "$expected" ]; then
      echo "cannot read the database schema version after migration" >&2
      exit 1
    fi
  done
fi

existing="$(psql "$DIAMOND_POSTGRES_URL" -Atc \
  "select count(*) from diamond_identity.users where username='${UI_USER}' and verified and not disabled")"
if [ "$existing" = "1" ]; then
  echo "account ${UI_USER} already exists; keeping it"
else
  printf '%s\n' "$UI_PASSWORD" | \
    "$DIAMOND" project-user-add build --postgres-env DIAMOND_POSTGRES_URL \
      --user "$UI_USER" --password-stdin
  echo "created account ${UI_USER}"
fi

echo "Serving http://127.0.0.1:${PORT}/"
exec "$RUNNER" app/diamond.toml "127.0.0.1:${PORT}" "http://127.0.0.1:${PORT}" \
  --mailbox "$MAILBOX"
