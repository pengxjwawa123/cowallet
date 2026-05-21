#!/bin/bash
# Wrapper entrypoint: syncs POSTGRES_PASSWORD to the database on every start,
# then delegates to the official postgres entrypoint.
# This fixes the issue where .env password changes don't take effect on existing volumes.

(
  until pg_isready -U "${POSTGRES_USER:-postgres}" -q; do
    sleep 1
  done
  if [ -n "$POSTGRES_PASSWORD" ]; then
    psql -U "${POSTGRES_USER:-postgres}" -d postgres -c \
      "ALTER USER ${POSTGRES_USER:-postgres} PASSWORD '${POSTGRES_PASSWORD}';" 2>/dev/null
  fi
) &

exec docker-entrypoint.sh "$@"
