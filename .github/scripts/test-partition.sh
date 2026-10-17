#!/usr/bin/env bash
# Gate: the partitioned test legs' runner and verifier, .github/scripts/partition.py.
#
# `partition.py run` records what one shard of `cargo test --all-targets
# --all-features` listed, selected and ran; `partition.py verify` is what makes
# the `test-coverage` job, and through it `upstroke-ci`, refuse a lane whose
# shards did not run every listed test exactly once. This gate runs both, as
# they ship, in throwaway repositories against a fake `cargo` that speaks
# libtest's listing and summary formats: every legitimate shape must pass and
# every refusal named in partition-fixtures.py must refuse for its own reason.
# It needs Python 3 and Git, and runs no Cargo, compiler or network.
set -euo pipefail
export PATH="/usr/bin:/bin:$PATH"

cd "$(dirname "${BASH_SOURCE[0]}")/../.."
exec python3 -I -B .github/scripts/partition-fixtures.py
