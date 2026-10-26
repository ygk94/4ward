#!/usr/bin/env bash
# Garde le testnet utilisable sans intervention (site Vercel, jury) : le mock-oracle ne publie rien tout seul et le
# contrat refuse un prix de plus de 900 s (erreur 302). Toutes les 10 min, repose le prix courant de chaque devise
# (crash.sh --refresh : même prix, nouvel horodatage ; un crash en cours est conservé).
#   ./scripts/keepalive.sh               rafraîchissement seul
#   ./scripts/keepalive.sh --keeper      + règle les couvertures échues (keeper.sh --once). PAS pendant la vidéo :
#                                        il réglerait la couverture de la démo avant le clic sur « Régler »
#   INTERVAL=300 ./scripts/keepalive.sh  période en secondes (défaut 600, doit rester sous 900)
# Lancement en arrière-plan qui survit au terminal : nohup caffeinate -i ./scripts/keepalive.sh >> keepalive.log 2>&1 &
# Arrêt : pkill -f scripts/keepalive.sh
set -uo pipefail
cd "$(dirname "$0")/.."

INTERVAL=${INTERVAL:-600}
KEEPER=0
[ "${1:-}" = "--keeper" ] && KEEPER=1

while :; do
  echo "[$(date '+%F %T')] rafraîchissement"
  for c in MXN BRL ARS EUR; do
    ./scripts/crash.sh --refresh "$c" >/dev/null 2>&1 && echo "  $c ok" || echo "  $c ÉCHEC"
  done
  if [ $KEEPER -eq 1 ]; then
    ./scripts/keeper.sh --once 2>&1 | grep -E "réglée|refus|échec" | sed 's/^/  /'
  fi
  sleep "$INTERVAL"
done
