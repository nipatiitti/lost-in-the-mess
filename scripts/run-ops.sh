#!/usr/bin/env bash
# scripts/run-ops.sh
set -euo pipefail
cargo build --bin ops
sudo setcap cap_net_raw,cap_net_admin=eip target/debug/ops
exec target/debug/ops "$@"