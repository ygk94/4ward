#!/usr/bin/env bash
# Réaligne l'offre DEX MXN/USDC (celle du setup, vendue par fx-admin) sur le prix MXN du mock-oracle.
# Sans ça, le path payment du fournisseur se fait à l'ancien prix et le coût total ne retombe plus sur le prix figé.
#   ./scripts/dex-align.sh            lit le prix MXN du mock, met à jour l'offre (manage-sell-offer, même id)
#   ./scripts/dex-align.sh --dry-run  calcule et affiche tout, sans rien envoyer
# Appelé par keepalive.sh --live après chaque recopie du cours Reflector. Procédure manuelle : docs/payment-cli.md § 5.
# Alias surchargeables : ORACLE=fx-mock-oracle ADMIN=fx-admin NET=testnet HORIZON=https://horizon-testnet.stellar.org
set -euo pipefail
cd "$(dirname "$0")/.."

usage() { sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; }

say() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
die() { printf '\033[31mErreur : %s\033[0m\n' "$*" >&2; exit 1; }

DRY_RUN=0
for arg in "$@"; do
  case "$arg" in
    -h|--help) usage; exit 0 ;;
    --dry-run) DRY_RUN=1 ;;
    *) die "option inconnue : $arg (voir --help)" ;;
  esac
done

[ -f .env.testnet ] || die ".env.testnet introuvable (lancer d'abord scripts/setup-testnet.sh)"
# shellcheck disable=SC1091
source .env.testnet
command -v jq >/dev/null || die "jq introuvable (brew install jq)"

NET=${NET:-testnet}
ADMIN=${ADMIN:-fx-admin}
HORIZON=${HORIZON:-https://horizon-testnet.stellar.org}
if [ -z "${ORACLE:-}" ]; then
  ORACLE=fx-mock-oracle
  stellar contract alias show "$ORACLE" --network "$NET" >/dev/null 2>&1 || ORACLE=$MOCK_ORACLE_ID
fi
# Lecture en simulation : une adresse source suffit (fx-admin peut manquer en --dry-run).
READER=$(stellar keys address "$ADMIN" 2>/dev/null || echo "$ISSUER")

# ---- prix du mock ----
OUT=$(stellar contract invoke --id "$ORACLE" --network "$NET" --source-account "$READER" --send=no -- \
  lastprice --asset '{"Other":"MXN"}' 2>/dev/null) || die "lecture de lastprice impossible sur $ORACLE"
[ -n "$OUT" ] && [ "$OUT" != null ] || die "pas de prix MXN sur le mock"
P=$(jq -r .price <<<"$OUT")
# Prix d'offre Stellar = fraction d'entiers 32 bits : P / 10⁷ : 10⁷ garde 7 décimales (0,0532524).
NUM=$(( P / 10000000 ))
[ "$NUM" -gt 0 ] || die "prix MXN trop petit pour une offre à 7 décimales ($P)"

# ---- offre actuelle ----
OFFER=$(curl -fsS -m 15 "$HORIZON/accounts/$ISSUER/offers?limit=200" \
  | jq -r '[._embedded.records[] | select(.selling.asset_code == "MXN" and .buying.asset_code == "USDC")][0] | "\(.id) \(.price)"' 2>/dev/null) \
  || die "lecture des offres de $ISSUER impossible sur Horizon"
read -r OFFER_ID OFFER_PRICE <<<"$OFFER"
[ -n "$OFFER_ID" ] && [ "$OFFER_ID" != null ] || die "aucune offre MXN/USDC de $ISSUER : relancer scripts/setup-testnet.sh"

say "Offre DEX MXN/USDC $OFFER_ID"
echo "   Prix de l'offre : $OFFER_PRICE USDC"
echo "   Prix du mock    : $(printf '0.%07d' "$NUM") USDC ($P)"
if [ "$OFFER_PRICE" = "$(printf '0.%07d' "$NUM")" ]; then
  echo "   Déjà aligné : rien à faire"
  exit 0
fi
echo "   manage-sell-offer --price $NUM:10000000 --offer-id $OFFER_ID"

if [ "$DRY_RUN" = 1 ]; then
  say "--dry-run : rien n'est envoyé"
  exit 0
fi

stellar keys address "$ADMIN" >/dev/null 2>&1 \
  || die "identité $ADMIN absente de cette machine : seul le vendeur de l'offre peut la modifier"
stellar tx new manage-sell-offer --source-account "$ADMIN" --network "$NET" --quiet \
  --selling "MXN:$ISSUER" --buying "USDC:$ISSUER" --amount 1000000000000000 \
  --price "$NUM:10000000" --offer-id "$OFFER_ID" >/dev/null \
  || die "manage-sell-offer a échoué"
echo "   ✅ offre réalignée à $(printf '0.%07d' "$NUM") USDC"
