#!/usr/bin/env bash
# Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
# Filet de sécurité de la vidéo : fait bouger le prix d'une devise sur le mock-oracle (bouton crash du front en panne).
#   ./scripts/crash.sh [PCT] [DEVISE]      crash : prix × (1 + PCT %), PCT défaut 12 (accepte -8, 12.5 ou 12,5), DEVISE défaut MXN
#   ./scripts/crash.sh --refresh [DEVISE]  repose le même prix (horodatage à maintenant, prix de moins de 900 s)
#   ./scripts/crash.sh --reset [DEVISE]    aligne le mock sur le vrai cours Reflector FX (PAS le prix de la vidéo)
#   ./scripts/crash.sh --demo [DEVISE]     pose le prix de la démo (setup) : MXN 0,055, BRL 0,18, ARS 0,001, EUR 1,1193
#   ./scripts/crash.sh --dry-run ...       calcule et affiche tout, sans envoyer set_price
# Alias surchargeables : ORACLE=fx-mock-oracle ADMIN=fx-admin NET=testnet (ex. ORACLE=pricing-mock ADMIN=pricing-dev).
set -euo pipefail
cd "$(dirname "$0")/.."

usage() { sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; }

say() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
die() { printf '\033[31mErreur : %s\033[0m\n' "$*" >&2; exit 1; }

# PCT (12, -8, 12.5, 12.25) → bps entiers, sans flottants.
pct_to_bps() {
  local p=${1/,/.} sign=1 int frac
  [[ $p =~ ^-?[0-9]+(\.[0-9]{1,2})?$ ]] || die "pourcentage invalide : $p (ex. 12, -8, 12.5)"
  if [[ $p == -* ]]; then sign=-1; p=${p#-}; fi
  int=${p%%.*}
  frac=0
  if [[ $p == *.* ]]; then frac=${p#*.}; [ ${#frac} -eq 1 ] && frac="${frac}0"; fi
  echo $(( sign * (10#$int * 100 + 10#$frac) ))
}

# ---- arguments ----
MODE=crash
DRY_RUN=0
PCT=12
CUR_SYM=MXN
for arg in "$@"; do
  case "$arg" in
    -h|--help) usage; exit 0 ;;
    --refresh) MODE=refresh ;;
    --reset) MODE=reset ;;
    --demo) MODE=demo ;;
    --dry-run) DRY_RUN=1 ;;
    -[0-9]*|[0-9]*) PCT=$arg ;;
    -*) die "option inconnue : $arg (voir --help)" ;;
    *) CUR_SYM=$(printf '%s' "$arg" | tr '[:lower:]' '[:upper:]') ;;
  esac
done

[ "$MODE" != crash ] || BPS=$(pct_to_bps "$PCT")

[ -f .env.testnet ] || die ".env.testnet introuvable (lancer d'abord scripts/setup-testnet.sh)"
# shellcheck disable=SC1091
source .env.testnet
command -v jq >/dev/null || die "jq introuvable (brew install jq)"

NET=${NET:-testnet}
ADMIN=${ADMIN:-fx-admin}
# Alias local du mock ; repli sur l'ID de .env.testnet si l'alias n'existe pas sur cette machine.
if [ -z "${ORACLE:-}" ]; then
  ORACLE=fx-mock-oracle
  stellar contract alias show "$ORACLE" --network "$NET" >/dev/null 2>&1 || ORACLE=$MOCK_ORACLE_ID
fi
# Offre DEX MXN/USDC posée par setup-testnet.sh (1 MXN = 0,055 USDC), en 14 décimales.
DEX_PRICE=${DEX_PRICE:-5500000000000}
# Lectures en simulation : il suffit d'une adresse source, pas d'une clé (fx-admin peut manquer sur la machine).
READER=$(stellar keys address "$ADMIN" 2>/dev/null || echo "$ISSUER")
ASSET="{\"Other\":\"$CUR_SYM\"}"

# ---- helpers ----

# Prix 14 décimales → « 0,0550 » (arrondi à 4 décimales).
fmt_price() {
  local v=$(( ($1 + 5000000000) / 10000000000 ))
  printf '%d,%04d' $(( v / 10000 )) $(( v % 10000 ))
}

# bps → « +12 % », « -8 % », « +12,5 % ».
fmt_pct() {
  local b=$1 sign=+ frac
  if [ "$b" -lt 0 ]; then sign=-; b=$(( -b )); fi
  frac=$(printf '%02d' $(( b % 100 ))); frac=${frac%0}
  if [ "$frac" = 0 ] || [ -z "$frac" ]; then printf '%s%d %%' "$sign" $(( b / 100 ))
  else printf '%s%d,%s %%' "$sign" $(( b / 100 )) "$frac"; fi
}

# Écart relatif en bps, arrondi : (b − a) / a.
delta_bps() {
  local d=$(( ($2 - $1) * 10000 )) r
  if [ "$d" -ge 0 ]; then r=$(( (d + $1 / 2) / $1 )); else r=$(( (d - $1 / 2) / $1 )); fi
  echo "$r"
}

# Montant en centimes → « 55 012,81 ».
fmt_usd() {
  local c=$1 d u=""
  d=$(( c / 100 ))
  while [ "$d" -ge 1000 ]; do u=$(printf ' %03d' $(( d % 1000 )))$u; d=$(( d / 1000 )); done
  printf '%d%s,%02d' "$d" "$u" $(( c % 100 ))
}

# lastprice en simulation → « prix horodatage », ou rien si pas de prix.
lastprice() {
  local out
  out=$(stellar contract invoke --id "$1" --network "$NET" --source-account "$READER" --send=no -- \
    lastprice --asset "$ASSET" 2>/dev/null) || die "lecture de lastprice impossible sur $1 ($CUR_SYM)"
  if [ "$out" = null ] || [ -z "$out" ]; then return 0; fi
  printf '%s %s\n' "$(jq -r .price <<<"$out")" "$(jq -r .timestamp <<<"$out")"
}

show_price() { # libellé prix horodatage
  local now age
  now=$(date +%s); age=$(( now - $3 ))
  echo "   $1 : $(fmt_price "$2") \$ ($2), posé à $(date -r "$3" '+%H:%M:%S' 2>/dev/null || date -d "@$3" '+%H:%M:%S'), il y a ${age} s"
  if [ "$age" -gt 900 ]; then echo "   ⚠️  plus de 900 s : quote / open_hedge le refuseront (stale price)"; fi
}

# ---- prix courant du mock ----
say "Mock-oracle $ORACLE, $CUR_SYM ($NET)"
OUT=$(lastprice "$ORACLE")
read -r CUR CUR_TS <<<"$OUT" || true
if [ -z "${CUR:-}" ]; then
  [ "$MODE" = reset ] || [ "$MODE" = demo ] || die "pas de prix $CUR_SYM sur le mock : lancer d'abord --reset (prix Reflector) ou set_price à la main"
  echo "   pas encore de prix $CUR_SYM sur le mock"
else
  show_price "Prix actuel" "$CUR" "$CUR_TS"
fi

# ---- nouveau prix ----
case "$MODE" in
  crash)
    [ "$BPS" -gt -10000 ] || die "baisse de 100 % ou plus : le prix doit rester positif"
    NEW=$(( CUR * (10000 + BPS) / 10000 ))
    LABEL="Crash $(fmt_pct "$BPS")"
    ;;
  refresh)
    NEW=$CUR
    LABEL="Rafraîchissement (même prix, nouvel horodatage)"
    ;;
  reset)
    say "Reflector FX $REFLECTOR_FX_ID"
    OUT=$(lastprice "$REFLECTOR_FX_ID")
    read -r REF REF_TS <<<"$OUT" || true
    [ -n "${REF:-}" ] || die "Reflector n'a pas de prix $CUR_SYM"
    show_price "Cours réel" "$REF" "$REF_TS"
    NEW=$REF
    LABEL="Alignement sur Reflector"
    ;;
  demo)
    # Prix posés par setup-testnet.sh (14 décimales) : ceux des chiffres de la vidéo.
    case "$CUR_SYM" in
      MXN) NEW=5500000000000 ;;
      BRL) NEW=18000000000000 ;;
      ARS) NEW=100000000000 ;;
      EUR) NEW=111930000000000 ;;
      *) die "pas de prix de démo pour $CUR_SYM (MXN, BRL, ARS, EUR)" ;;
    esac
    LABEL="Prix de la démo (setup-testnet.sh)"
    ;;
esac

say "$LABEL"
if [ -n "${CUR:-}" ]; then
  echo "   $CUR_SYM : $(fmt_price "$CUR") \$ → $(fmt_price "$NEW") \$ ($(fmt_pct "$(delta_bps "$CUR" "$NEW")"))"
fi
echo "   set_price --asset '$ASSET' --price $NEW"

if [ "$MODE" = reset ] && [ "$CUR_SYM" = MXN ]; then
  GAP=$(delta_bps "$DEX_PRICE" "$NEW")
  echo "   Écart avec le prix DEX de référence du setup ($(fmt_price "$DEX_PRICE"), vérifier l'offre réelle sur Horizon) : $(fmt_pct "$GAP")"
  if [ "$GAP" -ne 0 ]; then
    echo "   → prévenir Hugo / Clément : réaligner l'offre DEX (docs/payment-cli.md § 5)."
    # Chiffres de la vidéo à 0,055, cotés sur 90 j (facteur 25 920) : prix figé 55 012,81 · payé au total après +12 %
    # 56 098,72 (taux de scripts/rates.snapshot). Proportionnels au spot à durée égale.
    R9=$(( NEW / 1000 ))   # ≈ 5,3e9 pour MXN : × 5,6e6 ≈ 3e16, tient en 64 bits
    echo "   Chiffres vidéo recalculés au prorata du spot : prix figé ≈ $(fmt_usd $(( (5501281 * R9 + 2750000000) / 5500000000 ))) USDC," \
      "payé au total ≈ $(fmt_usd $(( (5609872 * R9 + 2750000000) / 5500000000 ))) USDC (à confirmer par quote)"
  fi
fi

# ---- écriture ----
if [ "$DRY_RUN" = 1 ]; then
  say "--dry-run : rien n'est envoyé"
else
  stellar keys address "$ADMIN" >/dev/null 2>&1 \
    || die "identité $ADMIN absente de cette machine : seul l'admin du mock peut poser un prix (ADMIN=... pour en choisir une autre)"
  LOG=$(mktemp)
  trap 'rm -f "$LOG"' EXIT
  say "Envoi de set_price (signé par $ADMIN)"
  if ! stellar contract invoke --id "$ORACLE" --network "$NET" --source-account "$ADMIN" -- \
      set_price --asset "$ASSET" --price "$NEW" >/dev/null 2>"$LOG"; then
    cat "$LOG" >&2
    die "set_price a échoué (l'identité $ADMIN est-elle bien l'admin du mock ?)"
  fi
  LINK=$(grep -Eo 'https://stellar\.expert/explorer/[a-z]+/tx/[0-9a-f]+' "$LOG" | head -1 || true)
  if [ -z "$LINK" ]; then
    HASH=$(grep -Eo '\b[0-9a-f]{64}\b' "$LOG" | head -1 || true)
    [ -n "$HASH" ] && LINK="https://stellar.expert/explorer/$NET/tx/$HASH"
  fi

  say "Vérification"
  OUT=$(lastprice "$ORACLE")
  read -r AFTER AFTER_TS <<<"$OUT" || true
  [ "${AFTER:-}" = "$NEW" ] || die "le mock renvoie ${AFTER:-rien} au lieu de $NEW"
  if [ -n "${CUR:-}" ]; then
    [ "$CUR_SYM" = MXN ] && NAME=Peso || NAME=$CUR_SYM
    echo "   $NAME : $(fmt_price "$CUR") \$ → $(fmt_price "$AFTER") \$ ($(fmt_pct "$(delta_bps "$CUR" "$AFTER")"))"
  fi
  show_price "Prix posé" "$AFTER" "$AFTER_TS"
  [ -n "$LINK" ] && echo "   Transaction : $LINK"
fi

cat <<'TXT'

Rappel démo (fixing à l'échéance, facteur d'accélération 25 920 : 90 jours = 5 min réelles)
  - Le crash ne compte que s'il est posé AVANT l'échéance réelle de la couverture.
  - settle prend le prix en vigueur à l'échéance (pricing::fixing) : inutile de rafraîchir avant settle,
    mais ce prix doit avoir moins de 900 s à l'échéance.
  - open_hedge lit le prix courant : lancer --refresh juste avant (prix de moins de 900 s).
TXT
