#!/usr/bin/env bash
set -euo pipefail
NIC="${1:?usage: $0 <iface> [channel]}"
CH="${2:-36}"
sudo nmcli dev set "$NIC" managed no
sudo ip link set "$NIC" down
sudo iw dev "$NIC" set type monitor
sudo ip link set "$NIC" up
sudo iw dev "$NIC" set channel "$CH" HT40+
sudo iw dev "$NIC" set power_save off
iw dev "$NIC" info