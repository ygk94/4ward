#!/usr/bin/env bash
# Compare le cours MXN du mock-oracle de la démo au vrai Reflector FX testnet (même interface SEP-40).
#   ./scripts/oracles.sh [DEVISE]   (MXN par défaut)
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
source .env.testnet
C=${1:-MXN}
price() { stellar contract invoke --id "$1" --source-account fx-lp --network testnet -- lastprice --asset "{\"Other\":\"$C\"}" 2>/dev/null \
  | python3 -c "import json,sys; print(f'{int(json.load(sys.stdin)[\"price\"]) / 1e14:.5f} USD')"; }
printf '%-34s %s = %s\n' "Demo oracle (mock, SEP-40)" "$C" "$(price "$MOCK_ORACLE_ID")"
printf '%-34s %s = %s\n' "Reflector FX testnet (real)" "$C" "$(price "$REFLECTOR_FX_ID")"
