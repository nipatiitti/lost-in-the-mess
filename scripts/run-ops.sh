#!/usr/bin/env bash
# scripts/run-ops.sh
set -euo pipefail
cargo build --bin litm-app
sudo setcap cap_net_raw,cap_net_admin=eip target/debug/litm-app
exec target/debug/litm-app "$@"