#!/usr/bin/env bash
# Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
# Garde le testnet utilisable sans intervention (site Vercel, jury) : le mock-oracle ne publie rien tout seul et le
# contrat refuse un prix de plus de 900 s (erreur 302). Toutes les 10 min, repose le prix courant de chaque devise
# (crash.sh --refresh : même prix, nouvel horodatage ; un crash en cours est conservé).
#   ./scripts/keepalive.sh               rafraîchissement seul
#   ./scripts/keepalive.sh --keeper      + règle les couvertures échues (keeper.sh --once). PAS pendant la vidéo :
#                                        il réglerait la couverture de la démo avant le clic sur « Régler »
#   ./scripts/keepalive.sh --live        recopie le cours Reflector FX dans le mock (crash.sh --reset) au lieu de
#                                        reposer le même prix, puis réaligne l'offre DEX MXN (dex-align.sh). Site public
#                                        au cours du jour ; un crash ne dure que jusqu'au passage suivant. PAS pendant la
#                                        vidéo (chiffres à 0,055). Combinable : --live --keeper
#   INTERVAL=300 ./scripts/keepalive.sh  période en secondes (défaut 600, doit rester sous 900)
# Lancement en arrière-plan qui survit au terminal : nohup caffeinate -i ./scripts/keepalive.sh >> keepalive.log 2>&1 &
# Arrêt : pkill -f scripts/keepalive.sh
set -uo pipefail
cd "$(dirname "$0")/.."

INTERVAL=${INTERVAL:-600}
KEEPER=0
LIVE=0
for arg in "$@"; do
  case "$arg" in
    --keeper) KEEPER=1 ;;
    --live) LIVE=1 ;;
    *) echo "option inconnue : $arg (--keeper, --live)" >&2; exit 1 ;;
  esac
done
MODE=--refresh
[ $LIVE -eq 1 ] && MODE=--reset

while :; do
  echo "[$(date '+%F %T')] rafraîchissement$([ $LIVE -eq 1 ] && echo ' (cours Reflector)')"
  for c in MXN BRL ARS EUR; do
    ./scripts/crash.sh $MODE "$c" >/dev/null 2>&1 && echo "  $c ok" || echo "  $c ÉCHEC"
  done
  if [ $LIVE -eq 1 ]; then
    ./scripts/dex-align.sh 2>&1 | grep -E "✅|aligné|Erreur" | sed 's/^ */  DEX : /'
  fi
  if [ $KEEPER -eq 1 ]; then
    ./scripts/keeper.sh --once 2>&1 | grep -E "réglée|refus|échec" | sed 's/^/  /'
  fi
  sleep "$INTERVAL"
done
