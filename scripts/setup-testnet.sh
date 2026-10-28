#!/usr/bin/env bash
# Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
# Monte tout l'environnement de démo sur le testnet Stellar.
# ATTENTION : chaque lancement redéploie mock-oracle et pool (nouveaux IDs, .env.testnet et front/.env.local réécrits,
# couvertures et historique perdus pour le front). Pour remettre un prix : ./scripts/crash.sh --demo (set_price).
#   ./scripts/setup-testnet.sh           tout le setup (comptes, actifs, offre DEX, contrats)
#   ./scripts/setup-testnet.sh --help    cette aide (tout autre argument est refusé)
# Résultat : IDs de contrats et d'actifs dans .env.testnet et front/.env.local
set -euo pipefail
cd "$(dirname "$0")/.."

case "${1:-}" in
  -h|--help) sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
  '') ;;
  *) echo "option inconnue : $1 (voir --help)" >&2; exit 1 ;;
esac

NET=testnet
ADMIN=fx-admin      # émetteur des actifs de test + admin des contrats
LP=fx-lp            # fournisseur de liquidité (l'équipe amorce le pool)
USER=fx-lucia       # la PME (persona de la démo)
SUPPLIER=fx-supplier # le fournisseur mexicain, reçoit des MXN

# Actifs classiques de test émis par ADMIN. On n'utilise pas l'USDC Circle :
# le faucet donne 20 USDC / 2 h, trop peu pour une facture de 1 000 000 MXN.
SPREAD_BPS=50

# Python : python3, sinon python (sous Windows, « python3 » peut être un raccourci vers le Microsoft Store qui ne fait rien)
PY=""
for cand in python3 python; do
  if "$cand" -c "import sys; sys.exit(0 if sys.version_info[0] == 3 else 1)" >/dev/null 2>&1; then PY=$cand; break; fi
done
[ -n "$PY" ] || { echo "Python 3 introuvable (essayé : python3, python). Installe-le puis relance." >&2; exit 1; }

say() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
tx() { stellar tx new "$@" --network $NET --quiet >/dev/null 2>&1 || echo "   (déjà fait ou échec non bloquant : $1)"; }

say "Comptes (générés et financés par friendbot)"
for k in $ADMIN $LP $USER $SUPPLIER; do
  stellar keys address $k >/dev/null 2>&1 || stellar keys generate $k --network $NET --fund --quiet
  echo "   $k = $(stellar keys address $k)"
done
ISSUER=$(stellar keys address $ADMIN)
USDC="USDC:$ISSUER"
MXN="MXN:$ISSUER"

say "Trustlines"
tx change-trust --source-account $LP --line "$USDC"
tx change-trust --source-account $USER --line "$USDC"
tx change-trust --source-account $SUPPLIER --line "$MXN"

say "Distribution d'USDC de test"
tx payment --source-account $ADMIN --destination $LP --asset "$USDC" --amount 10000000000000      # 1 000 000 USDC
tx payment --source-account $ADMIN --destination $USER --asset "$USDC" --amount 2000000000000     # 200 000 USDC

say "Offre DEX MXN/USDC pour le path payment (1 MXN = 0.055 USDC)"
# Une seule offre : on met à jour l'existante (sinon chaque lancement en empile une nouvelle à 0.055,
# et le path payment prend toujours la moins chère, même après un réalignement du prix).
OFFERS=$(curl -s "https://horizon-testnet.stellar.org/accounts/$ISSUER/offers?limit=200" | "$PY" -c "
import json, sys
recs = json.load(sys.stdin).get('_embedded', {}).get('records', [])
print(' '.join(o['id'] for o in recs if o['selling'].get('asset_code') == 'MXN' and o['buying'].get('asset_code') == 'USDC'))")
set -- $OFFERS
OFFER_ID=${1:-0}
tx manage-sell-offer --source-account $ADMIN --selling "$MXN" --buying "$USDC" \
  --amount 1000000000000000 --price "11:200" --offer-id $OFFER_ID                                   # 100 000 000 MXN
shift 2>/dev/null || true
for extra in "$@"; do   # doublons laissés par d'anciennes versions du script
  tx manage-sell-offer --source-account $ADMIN --selling "$MXN" --buying "$USDC" --amount 0 --price "11:200" --offer-id $extra
done

say "Contrats d'actifs (SAC)"
stellar contract asset deploy --asset "$USDC" --source-account $ADMIN --network $NET --quiet >/dev/null 2>&1 || true
stellar contract asset deploy --asset "$MXN" --source-account $ADMIN --network $NET --quiet >/dev/null 2>&1 || true
USDC_SAC=$(stellar contract id asset --asset "$USDC" --network $NET)
MXN_SAC=$(stellar contract id asset --asset "$MXN" --network $NET)
echo "   USDC SAC = $USDC_SAC"
echo "   MXN SAC  = $MXN_SAC"

say "Build des contrats"
stellar contract build --quiet

say "Déploiement mock-oracle"
ORACLE=$(stellar contract deploy --wasm target/wasm32v1-none/release/mock_oracle.wasm \
  --source-account $ADMIN --network $NET --alias fx-mock-oracle --quiet -- --admin $ADMIN)
echo "   mock-oracle = $ORACLE"
# Prix en USD par unité, 14 décimales (même format que Reflector FX)
for p in "MXN 5500000000000" "BRL 18000000000000" "ARS 100000000000" "EUR 111930000000000"; do
  set -- $p
  stellar contract invoke --id $ORACLE --source-account $ADMIN --network $NET --quiet -- \
    set_price --asset "{\"Other\":\"$1\"}" --price $2 >/dev/null
done

say "Déploiement pool"
POOL=$(stellar contract deploy --wasm target/wasm32v1-none/release/pool.wasm \
  --source-account $ADMIN --network $NET --alias fx-pool --quiet -- \
  --admin $ADMIN --usdc $USDC_SAC --oracle $ORACLE --spread_bps $SPREAD_BPS)
echo "   pool = $POOL"

say "Paramètres Pricing (taux d'intérêt, plafonds minimum)"
# Taux annuels en bps, figés dans scripts/rates.snapshot (rendements Etherfuse au jour du snapshot, ARS en dur) :
# pas d'appel réseau ici. Le refaire exprès : ./scripts/sync-rates.sh --snapshot (les chiffres de la démo changent).
# Repli si le snapshot manque ou est vide : copie du snapshot du 25/10/2026 (chiffres de la démo : 55 012,81 USDC).
RATES=("USD 360" "MXN 555" "BRL 1148" "ARS 2200" "EUR 210")
RATES_SRC="copie du snapshot du 25/10/2026 (scripts/rates.snapshot absent ou vide)"
if [ -f scripts/rates.snapshot ]; then
  SNAP=()
  while read -r c b _; do
    if [[ $c =~ ^[A-Z]{3}$ && $b =~ ^[0-9]+$ ]]; then SNAP+=("$c $b"); fi
  done < scripts/rates.snapshot
  if [ ${#SNAP[@]} -gt 0 ]; then RATES=("${SNAP[@]}"); RATES_SRC="scripts/rates.snapshot"; fi
fi
echo "   taux : $RATES_SRC"
for r in "${RATES[@]}"; do
  set -- $r
  stellar contract invoke --id $POOL --source-account $ADMIN --network $NET --quiet -- \
    set_rate --currency $1 --rate_bps $2 >/dev/null
  echo "   $1 = $2 bps"
done
# Plafond minimum par devise. LISTE BLANCHE : une devise sans set_min_cap n'est PAS cotable
# (quote et open_hedge refusent la devise).
for c in "MXN 500" "BRL 700" "ARS 1500" "EUR 300"; do
  set -- $c
  stellar contract invoke --id $POOL --source-account $ADMIN --network $NET --quiet -- \
    set_min_cap --currency $1 --cap_bps $2 >/dev/null
done

say "Mode démo : 90 jours en 5 minutes (time_factor)"
# Échéance réelle = durée annoncée / DEMO_TIME_FACTOR ; le prix reste calculé sur la durée annoncée.
# 25920 : 90 jours (7 776 000 s) → 300 s. DEMO_TIME_FACTOR=1 pour le temps réel.
DEMO_TIME_FACTOR=${DEMO_TIME_FACTOR:-25920}
stellar contract invoke --id $POOL --source-account $ADMIN --network $NET --quiet -- \
  set_time_factor --factor $DEMO_TIME_FACTOR >/dev/null
echo "   time_factor = $DEMO_TIME_FACTOR"

say "Dépôt du LP dans le pool (1 000 000 USDC contre des parts)"
LP_DEPOSIT=10000000000000   # 1 000 000 USDC, 7 décimales
LP_USDC=$(stellar contract invoke --id $USDC_SAC --source-account $LP --network $NET --quiet -- \
  balance --id $LP | tr -d '"')
if [ "$LP_USDC" -lt "$LP_DEPOSIT" ]; then
  echo "   $LP n'a que $LP_USDC stroops d'USDC (il en faut $LP_DEPOSIT) : dépôt sauté."
  echo "   Relancer le script (il renvoie de l'USDC au LP) ou déposer à la main avec deposit_lp."
else
  SHARES=$(stellar contract invoke --id $POOL --source-account $LP --network $NET --quiet -- \
    deposit_lp --lp $LP --amount $LP_DEPOSIT | tr -d '"')
  echo "   parts de $LP = $SHARES"
fi

REFLECTOR_FX=CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W
cat > .env.testnet <<ENV
NETWORK=testnet
RPC_URL=https://soroban-testnet.stellar.org
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
ISSUER=$ISSUER
USDC_SAC=$USDC_SAC
MXN_SAC=$MXN_SAC
POOL_ID=$POOL
MOCK_ORACLE_ID=$ORACLE
REFLECTOR_FX_ID=$REFLECTOR_FX
LP=$(stellar keys address $LP)
USER=$(stellar keys address $USER)
SUPPLIER=$(stellar keys address $SUPPLIER)
ENV
sed -e 's/^/VITE_/' .env.testnet > front/.env.local
say "OK, IDs écrits dans .env.testnet et front/.env.local"
