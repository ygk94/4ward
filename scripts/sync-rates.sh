#!/usr/bin/env bash
# Push oracle des taux : remplace les taux d'intérêt en dur du pool par le rendement des stablebonds Etherfuse (set_rate).
#   ./scripts/sync-rates.sh            USD ← USTRY, MXN ← CETES, BRL ← TESOURO, EUR ← EUROB ; ARS garde son taux en dur
#   ./scripts/sync-rates.sh --all      ajoute GBP ← GILTS
#   ./scripts/sync-rates.sh --dry-run  calcule et affiche tout, sans envoyer set_rate
# Repli sur le taux en dur de la devise si l'API ne répond pas, si la valeur manque, vaut 0 ou sort de ]0, 5000] bps.
# Alias surchargeables : POOL=fx-pool ADMIN=fx-admin NET=testnet ETHERFUSE_URL=https://api.etherfuse.com/lookup/tokens/cost
set -euo pipefail
cd "$(dirname "$0")/.."

usage() { sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; }

say() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
warn() { printf '\033[33m   %s\033[0m\n' "$*"; }
die() { printf '\033[31mErreur : %s\033[0m\n' "$*" >&2; exit 1; }

# ---- arguments ----
DRY_RUN=0
ALL=0
for arg in "$@"; do
  case "$arg" in
    -h|--help) usage; exit 0 ;;
    --dry-run) DRY_RUN=1 ;;
    --all) ALL=1 ;;
    *) die "option inconnue : $arg (voir --help)" ;;
  esac
done

# .env.testnet est facultatif : il ne sert qu'au repli sur POOL_ID (setup-testnet.sh appelle ce script avant de l'écrire).
# shellcheck disable=SC1091
[ -f .env.testnet ] && source .env.testnet

NET=${NET:-testnet}
ADMIN=${ADMIN:-fx-admin}
ETHERFUSE_URL=${ETHERFUSE_URL:-https://api.etherfuse.com/lookup/tokens/cost}
MAX_BPS=5000   # garde-fou : le contrat refuse au-delà (RateTooHigh)
# Alias local du pool ; repli sur l'ID de .env.testnet si l'alias n'existe pas sur cette machine.
if [ -z "${POOL:-}" ]; then
  POOL=fx-pool
  stellar contract alias show "$POOL" --network "$NET" >/dev/null 2>&1 || POOL=${POOL_ID:-}
fi
[ -n "$POOL" ] || [ "$DRY_RUN" = 1 ] || die "pool introuvable : ni alias fx-pool ni POOL_ID dans .env.testnet (POOL=... pour le choisir)"

# Devise, stablebond Etherfuse (- : aucun), taux en dur de référence en bps (- : aucun).
# Taux en dur = ceux de setup-testnet.sh (Fed, Banxico, Selic, proxy BADLAR, BCE) : docs/todo-pricing.md §1.5
ROWS=("USD USTRY 400" "MXN CETES 700" "BRL TESOURO 1050" "ARS - 2200" "EUR EUROB 200")
[ "$ALL" = 0 ] || ROWS+=("GBP GILTS -")

# bps → « 5,55 % ».
fmt_pct() { printf '%d,%02d %%' $(( $1 / 100 )) $(( $1 % 100 )); }

# ---- lecture Etherfuse ----
say "Rendements Etherfuse ($ETHERFUSE_URL)"
PY=""
for cand in python3 python; do
  if "$cand" -I -c "import sys; sys.exit(0 if sys.version_info[0] == 3 else 1)" >/dev/null 2>&1; then PY=$cand; break; fi
done

SYMBOLS=""
for r in "${ROWS[@]}"; do
  set -- $r
  [ "$2" = - ] || SYMBOLS="$SYMBOLS $2"
done

# Une ligne « SYMBOLE état valeur » par stablebond ; état : ok | panne | json | absent.
FETCHED=""
if [ -z "$PY" ]; then
  warn "Python 3 introuvable (essayé : python3, python) : taux en dur conservés"
elif ! BODY=$(curl -fsS -m 10 "$ETHERFUSE_URL" 2>&1); then
  warn "API injoignable ($BODY)"
else
  FETCHED=$(printf '%s' "$BODY" | "$PY" -I -c "
import json, sys
try:
    data = json.load(sys.stdin)
    assert isinstance(data, dict)
except Exception:
    for s in sys.argv[1:]: print(s, 'json', '-')
    sys.exit(0)
for s in sys.argv[1:]:
    tok = data.get(s)
    bps = tok.get('current_basis_points') if isinstance(tok, dict) else None
    if isinstance(bps, bool) or not isinstance(bps, int):
        print(s, 'absent', '-')
    else:
        print(s, 'ok', bps)
" $SYMBOLS)
fi

# ---- calcul des taux à pousser ----
# Lignes du tableau : devise stablebond en_dur etherfuse poussé source
TABLE=()
PUSH=()
NOTES=()
N_EF=0
N_FALLBACK=0
for r in "${ROWS[@]}"; do
  set -- $r
  CUR=$1 SYM=$2 HARD=$3
  if [ "$SYM" = - ]; then
    TABLE+=("$CUR - $HARD - $HARD en_dur")
    PUSH+=("$CUR $HARD")
    continue
  fi
  LINE=$(grep -E "^$SYM " <<<"$FETCHED" || true)
  STATE=panne VAL=-
  [ -z "$LINE" ] || read -r _ STATE VAL <<<"$LINE"
  REASON=""
  case "$STATE" in
    panne) REASON="API sans réponse" ;;
    json) REASON="JSON invalide" ;;
    absent) REASON="current_basis_points absent" ;;
    ok)
      if [ "$VAL" -eq 0 ]; then REASON="$SYM vaut 0"
      elif [ "$VAL" -lt 0 ] || [ "$VAL" -gt "$MAX_BPS" ]; then REASON="$SYM = $VAL bps, hors de ]0, $MAX_BPS] (le contrat refuserait)"
      fi ;;
  esac
  if [ -z "$REASON" ]; then
    TABLE+=("$CUR $SYM $HARD $VAL $VAL etherfuse")
    PUSH+=("$CUR $VAL")
    N_EF=$((N_EF + 1))
  elif [ "$HARD" = - ]; then
    TABLE+=("$CUR $SYM - $VAL - repli")
    NOTES+=("$CUR : $REASON, pas de taux en dur : set_rate sauté")
    N_FALLBACK=$((N_FALLBACK + 1))
  else
    TABLE+=("$CUR $SYM $HARD $VAL $HARD repli")
    PUSH+=("$CUR $HARD")
    NOTES+=("$CUR : $REASON, taux en dur conservé ($(fmt_pct "$HARD"))")
    N_FALLBACK=$((N_FALLBACK + 1))
  fi
done

# ---- tableau ----
cell() { if [ "$1" = - ]; then echo -; elif [ "$1" -lt 0 ]; then echo "$1 bps"; else fmt_pct "$1"; fi; }
echo
echo "   Devise  Stablebond  En dur    Etherfuse  Poussé    Source"
for t in "${TABLE[@]}"; do
  set -- $t
  ROW=$(printf '%-7s %-11s %-9s %-10s %-9s %s' "$1" "$2" "$(cell "$3")" "$(cell "$4")" "$(cell "$5")" "${6/_/ }")
  if [ "$6" = repli ]; then printf '\033[33m   %s\033[0m\n' "$ROW"; else echo "   $ROW"; fi
done
echo
for n in ${NOTES[@]+"${NOTES[@]}"}; do warn "⚠️  $n"; done
[[ " ${PUSH[*]} " != *" GBP "* ]] || echo "   GBP n'a pas de set_min_cap dans le setup : taux poussé, mais la devise reste non cotable."
if [ "$N_EF" -eq 0 ]; then
  warn "⚠️  Aucun taux Etherfuse utilisable : toutes les devises restent sur leurs taux en dur."
fi

# ---- écriture ----
if [ "$DRY_RUN" = 1 ]; then
  for p in "${PUSH[@]}"; do set -- $p; echo "   set_rate --currency $1 --rate_bps $2"; done
  say "--dry-run : rien n'est envoyé"
  exit 0
fi

stellar keys address "$ADMIN" >/dev/null 2>&1 \
  || die "identité $ADMIN absente de cette machine : seul l'admin du pool peut poser un taux (ADMIN=... pour en choisir une autre)"
LOG=$(mktemp)
trap 'rm -f "$LOG"' EXIT
say "Envoi de set_rate sur $POOL (signé par $ADMIN, $NET)"
FAILED=0
for p in "${PUSH[@]}"; do
  set -- $p
  if stellar contract invoke --id "$POOL" --source-account "$ADMIN" --network "$NET" -- \
      set_rate --currency "$1" --rate_bps "$2" >/dev/null 2>"$LOG"; then
    echo "   ✅ $1 = $2 bps ($(fmt_pct "$2"))"
  else
    printf '\033[31m   ❌ %s : set_rate a échoué\033[0m\n' "$1"
    sed 's/^/      /' "$LOG" >&2
    FAILED=$((FAILED + 1))
  fi
done
[ "$FAILED" -eq 0 ] || die "$FAILED appel(s) set_rate en échec (l'identité $ADMIN est-elle bien l'admin du pool ?)"

cat <<'TXT'

Pas de getter onchain pour les taux : vérifier l'effet avec un quote en simulation (le forward doit bouger), ex.
  stellar contract invoke --id fx-pool --network testnet --source-account fx-admin --send=no -- \
    quote --currency MXN --side BuyCurrency --notional 10000000000000 --duration_secs 7776000 --cap_bps 1000
TXT
