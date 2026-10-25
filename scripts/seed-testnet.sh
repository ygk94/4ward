#!/usr/bin/env bash
# Peuple le déploiement testnet avec de vrais comptes et de vraies transactions, pour tester le front et le contrat.
#   ./scripts/seed-testnet.sh            tout : comptes, dépôts LP, couvertures, refus attendus, mouvements de cours,
#                                        règlements par le keeper, retraits, contrôle de l'invariant
#   ./scripts/seed-testnet.sh --no-wait  sans attendre les échéances (pas de règlement ni de retrait final)
# À lancer après ./scripts/setup-testnet.sh (mode démo : 90 jours = 5 minutes). Relançable : les comptes existants
# sont réutilisés, chaque lancement ajoute de nouveaux dépôts et de nouvelles couvertures.
# Alias surchargeables : POOL=fx-pool ADMIN=fx-admin SUPPLIER=fx-supplier NET=testnet
set -uo pipefail
cd "$(dirname "$0")/.."

NET=${NET:-testnet}
POOL=${POOL:-fx-pool}
ADMIN=${ADMIN:-fx-admin}
SUPPLIER=${SUPPLIER:-fx-supplier}
HORIZON=https://horizon-testnet.stellar.org
WAIT=1
[ "${1:-}" = "--no-wait" ] && WAIT=0

PY=""
for cand in python3 python; do
  if "$cand" -c "import sys; sys.exit(0 if sys.version_info[0] == 3 else 1)" >/dev/null 2>&1; then PY=$cand; break; fi
done
[ -n "$PY" ] || { echo "Python 3 introuvable." >&2; exit 1; }

ISSUER=$(stellar keys address $ADMIN)
USDC="USDC:$ISSUER"
U=10000000            # 1 unité en 7 décimales
DAY=86400
OK=0; KO=0; EXPECTED=0

say() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
ok()  { OK=$((OK + 1)); printf '   \033[32m✓\033[0m %s\n' "$*"; }
ko()  { KO=$((KO + 1)); printf '   \033[31m✗ %s\033[0m\n' "$*"; }

# invoke SOURCE fonction args... : écrit le résultat (ou le code d'erreur) dans $OUT, renvoie 0 si succès
invoke() {
  local src=$1; shift
  OUT=$(stellar contract invoke --id $POOL --source-account $src --network $NET -- "$@" 2>&1)
  local rc=$?
  if [ $rc -ne 0 ]; then
    OUT=$(printf '%s' "$OUT" | grep -oE 'Error\(Contract, #[0-9]+\)' | head -1)
    [ -z "$OUT" ] && OUT="échec sans code"
    return 1
  fi
  OUT=$(printf '%s' "$OUT" | tail -1 | tr -d '"')
  return 0
}
read_json() { stellar contract invoke --id $POOL --source-account $ADMIN --network $NET --send=no -- "$@" 2>/dev/null | tail -1; }
read_pool() { read_json "$@" | tr -d '"'; }

ensure_account() {   # nom, USDC à recevoir
  local name=$1 usdc=$2
  if ! stellar keys address $name >/dev/null 2>&1; then
    stellar keys generate $name --network $NET --fund --quiet >/dev/null 2>&1 || { ko "$name : friendbot a refusé"; return 1; }
  fi
  stellar tx new change-trust --source-account $name --line "$USDC" --network $NET --quiet >/dev/null 2>&1
  stellar tx new payment --source-account $ADMIN --destination $name --asset "$USDC" \
    --amount $((usdc * U)) --network $NET --quiet >/dev/null 2>&1 \
    && ok "$name ($(stellar keys address $name | cut -c1-8)…) : +$usdc USDC" || ko "$name : envoi d'USDC refusé"
}

refresh_prices() {
  for c in MXN BRL EUR ARS; do ./scripts/crash.sh --refresh $c >/dev/null 2>&1 || ko "prix $c non rafraîchi"; done
}

HEDGES=()   # "id compte devise sens"
open() {      # compte devise sens notionnel jours plafond_bps [code_attendu]
  local who=$1 ccy=$2 side=$3 notional=$4 days=$5 cap=$6 expect=${7:-}
  if invoke $who open_hedge --owner $who --supplier $SUPPLIER --currency $ccy --side $side \
      --notional $((notional * U)) --duration_secs $((days * DAY)) --cap_bps $cap; then
    if [ -n "$expect" ]; then ko "$who $ccy : accepté alors qu'on attendait $expect"; return; fi
    HEDGES+=("$OUT $who $ccy $side")
    ok "couverture #$OUT : $who $side $notional $ccy, $days j, plafond $((cap / 100)) %"
  else
    if [ -n "$expect" ] && [[ "$OUT" == *"#$expect)"* ]]; then
      EXPECTED=$((EXPECTED + 1)); ok "refus attendu ($OUT) : $who $ccy, plafond $((cap / 100)) %, $notional"
    else
      ko "$who $ccy $side $notional : $OUT"
    fi
  fi
}

# ---------------------------------------------------------------------------------------------------------------
say "Comptes (vrais comptes testnet, financés par friendbot, USDC de test envoyés par $ADMIN)"
LPJ=(seed-lp-anna seed-lp-bruno seed-lp-chen)
LPS=(seed-lp-diane seed-lp-emre)
PME=(seed-pme-tequila seed-pme-cafe seed-pme-vins seed-pme-mate seed-pme-export seed-pme-soja)
for a in "${LPJ[@]}" "${LPS[@]}"; do ensure_account $a 300000; done
for a in "${PME[@]}"; do ensure_account $a 60000; done

say "Dépôts LP : tranche dynamique (junior) puis prudente (senior)"
dep() { invoke $1 $2 --lp $1 --amount $(($3 * U)) && ok "$1 : $2 $3 USDC → $OUT parts" || ko "$1 $2 $3 : $OUT"; }
dep seed-lp-anna  deposit_lp 200000
dep seed-lp-bruno deposit_lp 150000
dep seed-lp-chen  deposit_lp 50000
dep seed-lp-diane deposit_senior 100000
dep seed-lp-emre  deposit_senior 50000
# Le ratio dynamique minimum (20 %) laisse jusqu'à 4 × le dynamique en prudent : impossible à casser avec ces soldes
# (testé dans les tests du contrat). Ici : dépôt plus gros que le solde du wallet → refus du token USDC (#10).
if invoke seed-lp-diane deposit_senior --lp seed-lp-diane --amount $((2000000 * U)); then
  ko "dépôt de 2 M accepté alors que le wallet n'a que 200 000 USDC"
else
  [[ "$OUT" == *"#10)"* ]] && { EXPECTED=$((EXPECTED + 1)); ok "refus attendu ($OUT, solde insuffisant du token USDC) : dépôt plus gros que le wallet"; } || ko "dépôt 2 M : $OUT"
fi
echo "   tranches : $(read_pool tranche_values)"

say "Couvertures (prix du mock rafraîchis juste avant)"
refresh_prices
open seed-pme-tequila MXN BuyCurrency  2000000  30 1000
open seed-pme-cafe    BRL BuyCurrency   300000  60  800
open seed-pme-vins    EUR BuyCurrency    50000  90  500
open seed-pme-mate    ARS BuyCurrency 20000000  90 1500
open seed-pme-export  MXN SellCurrency 1500000  30 1000
open seed-pme-soja    BRL SellCurrency  500000 365 1000
open seed-pme-tequila MXN BuyCurrency   500000 365 2000

say "Refus attendus"
open seed-pme-mate ARS BuyCurrency 1000000 90 1000 307      # plafond sous le minimum ARS (15 %)
open seed-pme-vins JPY BuyCurrency 1000000 90 1000 306      # devise non activée
invoke $ADMIN set_currency_limit --currency ARS --limit_bps 100 >/dev/null && ok "limite ARS posée à 1 % du capital"
open seed-pme-mate ARS BuyCurrency 100000000 90 1500 108    # marge ≈ 15 000 USDC : dépasse la limite par devise
invoke $ADMIN set_currency_limit --currency ARS --limit_bps 10000 >/dev/null && ok "limite ARS remise à 100 %"

say "État après ouverture"
echo "   capacité : $(read_pool capacity) · total : $(read_pool total_assets)"
for c in MXN BRL EUR ARS; do echo "   exposition $c : $(read_pool exposure --currency $c)"; done

say "Mouvements de cours (avant les premières échéances)"
for m in "6 MXN" "-9 BRL" "2 EUR" "20 ARS"; do
  set -- $m; ./scripts/crash.sh $1 $2 >/dev/null 2>&1 && ok "$2 : $1 %" || ko "crash $2 $1 %"
done

if [ $WAIT -eq 1 ]; then
  say "Règlements par le keeper (30 j = 100 s, puis 60-90 j = 300 s)"
  sleep_until() { local t=$(( $(date +%s) + $1 )); while [ $(date +%s) -lt $t ]; do sleep 5; done; }
  sleep_until 110
  ./scripts/keeper.sh --once 2>&1 | grep -E "réglée|refus|rien" | sed 's/^/   /'
  refresh_prices   # le fixing des couvertures à 60-90 j doit trouver un prix récent à l'échéance
  sleep_until 200
  ./scripts/keeper.sh --once 2>&1 | grep -E "réglée|refus|rien" | sed 's/^/   /'

  say "Retraits LP"
  sh_chen=$(read_pool lp_shares --lp $(stellar keys address seed-lp-chen))
  invoke seed-lp-chen withdraw --lp seed-lp-chen --amount $((sh_chen / 2)) && ok "seed-lp-chen : la moitié de ses parts dynamiques → $OUT (USDC, 7 déc.)" || ko "retrait chen : $OUT"
  sh_emre=$(read_pool tranche_values >/dev/null; stellar contract invoke --id $POOL --source-account $ADMIN --network $NET --send=no -- senior_balance --lp seed-lp-emre 2>/dev/null | tail -1 | tr -d '"')
  echo "   valeur prudente de seed-lp-emre : $sh_emre"
fi

say "Contrôle de l'invariant sur la chaîne"
USDC_SAC=$(stellar contract id asset --asset "$USDC" --network $NET)
POOL_ADDR=$(stellar contract alias show $POOL --network $NET 2>/dev/null || echo $POOL)
BAL=$(stellar contract invoke --id $USDC_SAC --source-account $ADMIN --network $NET --send=no -- balance --id $POOL_ADDR 2>/dev/null | tail -1 | tr -d '"')
TOTAL=$(read_pool total_assets)
MARGINS=0; OPEN=0; id=0
while :; do
  h=$(read_json get_hedge --hedge_id $id)
  [ -z "$h" ] || [ "$h" = "null" ] && break
  m=$(printf '%s' "$h" | $PY -c "import json,sys; d=json.load(sys.stdin); print(0 if d['settled'] else d['margin'])")
  [ "$m" != "0" ] && { MARGINS=$((MARGINS + m)); OPEN=$((OPEN + 1)); }
  id=$((id + 1))
done
echo "   solde USDC du pool : $BAL"
echo "   capital des LPs    : $TOTAL"
echo "   marges ouvertes    : $MARGINS ($OPEN couverture(s) ouverte(s) sur $id)"
[ "$BAL" = "$((TOTAL + MARGINS))" ] && ok "invariant : solde = capital + marges ouvertes" || ko "invariant rompu : $BAL ≠ $((TOTAL + MARGINS))"
echo "   tranches : $(read_pool tranche_values) · capacité : $(read_pool capacity)"

say "Bilan : $OK ok (dont $EXPECTED refus attendus), $KO problème(s)"
[ $KO -eq 0 ]
