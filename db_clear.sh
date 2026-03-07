#!/usr/bin/env bash
# db_clear.sh - Clear all database tables for local testing
# Reads connection settings from sp_server/.env (falls back to defaults)

set -euo pipefail

# Load env vars from sp_server/.env if present
ENV_FILE="$(dirname "$0")/sp_server/.env"
if [[ -f "$ENV_FILE" ]]; then
    set -o allexport
    source "$ENV_FILE"
    set +o allexport
fi

DB_HOST="${DB_HOST:-localhost}"
DB_USER="${DB_USER:-postgres}"
DB_NAME="${DB_NAME:-postgres}"
export PGPASSWORD="${DB_PASSWORD:-}"

echo "Clearing all tables in database '$DB_NAME' on $DB_HOST as user '$DB_USER'..."

psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" <<'SQL'
BEGIN;

-- device_tokens references accounts, so delete it first
TRUNCATE TABLE device_tokens RESTART IDENTITY CASCADE;
TRUNCATE TABLE sessions;
TRUNCATE TABLE scores RESTART IDENTITY CASCADE;
TRUNCATE TABLE accounts RESTART IDENTITY CASCADE;

COMMIT;

SELECT 'accounts'     AS "table", COUNT(*) AS rows FROM accounts
UNION ALL
SELECT 'sessions',    COUNT(*) FROM sessions
UNION ALL
SELECT 'scores',      COUNT(*) FROM scores
UNION ALL
SELECT 'device_tokens', COUNT(*) FROM device_tokens;
SQL

echo "Done."
