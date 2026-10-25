#!/usr/bin/env bash
# Keeper : règle tout seul les couvertures arrivées à échéance (settle est ouvert à tous, sans signature de la PME).
#   ./scripts/keeper.sh                boucle : un passage toutes les 10 s (Ctrl+C pour arrêter)
#   ./scripts/keeper.sh --once         un seul passage, puis s'arrête
#   ./scripts/keeper.sh --interval 5   un passage toutes les 5 s
#   ./scripts/keeper.sh --dry-run      affiche les couvertures à régler, sans envoyer settle
# Alias surchargeables : POOL=fx-pool SOURCE=fx-admin NET=testnet HORIZON=https://horizon-testnet.stellar.org
# SOURCE ne fait que payer les frais : n'importe quel compte financé convient.
set -euo pipefail
cd "$(dirname "$0")/.."

usage() { sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; }

say() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
die() { printf '\033[31mErreur : %s\033[0m\n' "$*" >&2; exit 1; }

POOL=${POOL:-fx-pool}
SOURCE=${SOURCE:-fx-admin}
NET=${NET:-testnet}
HORIZON=${HORIZON:-https://horizon-testnet.stellar.org}
ONCE=0
DRY=0
INTERVAL=10

while [ $# -gt 0 ]; do
  case "$1" in
    --once) ONCE=1 ;;
    --dry-run) DRY=1 ;;
    --interval) shift; [[ ${1:-} =~ ^[0-9]+$ ]] && [ "$1" -gt 0 ] || die "--interval attend un nombre de secondes"; INTERVAL=$1 ;;
    -h|--help) usage; exit 0 ;;
    *) usage; die "option inconnue : $1" ;;
  esac
  shift
done

# Python : python3, sinon python (sous Windows, « python3 » peut être un raccourci vers le Microsoft Store qui ne fait rien).
# Ses sorties passent par « tr -d '\r' » : sous Windows, il termine les lignes par CRLF.
PY=""
for cand in python3 python; do
  if "$cand" -c 'import sys; sys.exit(0 if sys.version_info[0] == 3 else 1)' >/dev/null 2>&1; then PY=$cand; break; fi
done
[ -n "$PY" ] || die "Python 3 introuvable (essayé : python3, python). Installe-le puis relance."
command -v stellar >/dev/null || die "stellar CLI introuvable"

# Heure du dernier ledger fermé, en secondes Unix : c'est elle que settle compare à l'échéance.
ledger_time() {
  curl -fsS "$HORIZON/ledgers?order=desc&limit=1" | "$PY" -c '
import sys, json, datetime
r = json.load(sys.stdin)["_embedded"]["records"][0]
print(int(datetime.datetime.fromisoformat(r["closed_at"].replace("Z", "+00:00")).timestamp()))' | tr -d '\r'
}

# La couverture `id` en JSON, ou « null » si elle n'existe pas.
get_hedge() {
  stellar contract invoke --id "$POOL" --source-account "$SOURCE" --network "$NET" -- get_hedge --hedge_id "$1" 2>/dev/null
}

# Raison lisible d'un refus de settle, d'après le code d'erreur du contrat.
reason() {
  case "$1" in
    200) echo "couverture inconnue" ;;
    201) echo "déjà réglée (par quelqu'un d'autre entre-temps)" ;;
    202) echo "échéance pas encore atteinte côté réseau, nouvel essai au prochain passage" ;;
    302) echo "aucun cours publié dans les 15 min avant l'échéance" ;;
    312) echo "cours à l'échéance introuvable (historique de l'oracle dépassé)" ;;
    *) echo "voir les codes d'erreur dans docs/payment-cli.md" ;;
  esac
}

settle() {
  local id=$1 out code
  if out=$(stellar contract invoke --id "$POOL" --source-account "$SOURCE" --network "$NET" -- settle --hedge_id "$id" 2>&1); then
    echo "   couverture $id réglée : $(echo "$out" | tail -1 | tr -d '"\r') stroops versés à la PME"
    echo "$out" | grep -o 'https://stellar.expert[^ ]*' | sed 's/^/   /' || true
  else
    code=$(echo "$out" | grep -o 'Error(Contract, #[0-9]*)' | head -1 | grep -o '[0-9]*$' || true)
    echo "   couverture $id non réglée : ${code:+#$code }$(reason "${code:-?}")"
  fi
}

# Premier id dont on ne sait pas encore s'il est réglé : les couvertures réglées avant lui ne sont plus relues.
FIRST=0

pass() {
  local now id json settled maturity prefix=1 due=0
  now=$(ledger_time) || { echo "   heure du ledger illisible, passage sauté"; return; }
  id=$FIRST
  while :; do
    json=$(get_hedge "$id" || true)
    if [ -z "$json" ] || [ "$json" = null ]; then break; fi
    read -r settled maturity < <(echo "$json" | "$PY" -c 'import sys, json; h = json.load(sys.stdin); print(int(h["settled"]), h["maturity"])' | tr -d '\r')
    if [ "$settled" = 1 ]; then
      [ "$prefix" = 1 ] && FIRST=$((id + 1))
    else
      prefix=0
      if [ "$now" -ge "$maturity" ]; then
        due=$((due + 1))
        if [ "$DRY" = 1 ]; then echo "   couverture $id à régler (échue depuis $((now - maturity)) s)"; else settle "$id"; fi
      fi
    fi
    id=$((id + 1))
  done
  [ "$due" -gt 0 ] || echo "   rien à régler ($(date -u +%H:%M:%S) UTC, $id couverture(s) au total)"
}

say "Keeper sur $POOL ($NET), frais payés par $SOURCE$([ "$DRY" = 1 ] && echo ', à blanc')"
while :; do
  pass
  [ "$ONCE" = 1 ] && break
  sleep "$INTERVAL"
done
