#!/usr/bin/env bash
# Fails if SQL business logic constructs appear in migrations or Rust SQL strings.
set -euo pipefail
pattern='CREATE[[:space:]]+(TEMP[[:space:]]+|TEMPORARY[[:space:]]+)?(TRIGGER|VIEW)|PROCEDURE|create_(scalar_)?function|create_aggregate_function'
if grep -RInE --include='*.sql' --include='*.rs' "$pattern" crates apps 2>/dev/null; then
  echo "❌ SQL logic constructs are banned (see docs/adr/0002)." >&2
  exit 1
fi
echo "✅ No SQL triggers/views/procedures/UDFs found."
