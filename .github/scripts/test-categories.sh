#!/usr/bin/env bash
set -euo pipefail
root="$(cd "${BASH_SOURCE[0]%/*}/../.." && pwd)"
exec python3 "$root/.github/scripts/test-categories.py" check "$@"
