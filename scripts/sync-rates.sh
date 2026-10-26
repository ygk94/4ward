#!/usr/bin/env bash
# Push oracle des taux : remplace les taux d'intérêt en dur du pool par le rendement des stablebonds Etherfuse (set_rate).
#   ./scripts/sync-rates.sh            USD ← USTRY, MXN ← CETES, BRL ← TESOURO, EUR ← EUROB ; ARS garde son taux en dur
#   ./scripts/sync-rates.sh --all      ajoute GBP ← GILTS
#   ./scripts/sync-rates.sh --dry-run  calcule et affiche tout, sans envoyer set_rate
#   ./scripts/sync-rates.sh --snapshot écrit les taux dans scripts/rates.snapshot (lu par setup-testnet.sh), sans set_rate
#   ./scripts/sync-rates.sh --loop 3600 relance la synchro toutes les 3600 s (taux du pool en phase avec Etherfuse).
#                                       PAS pendant une vidéo : un taux qui bouge change les chiffres de la démo.
#                                       En arrière-plan : nohup ./scripts/sync-rates.sh --loop 3600 >> sync-rates.log 2>&1 &
# Repli sur le taux de scripts/rates.snapshot si l'API ne répond pas, si la valeur manque, vaut 0 ou sort de ]0, 5000] bps.
# Alias surchargeables : POOL=fx-pool ADMIN=fx-admin NET=testnet ETHERFUSE_URL=https://api.etherfuse.com/lookup/tokens/cost
set -euo pipefail
cd "$(dirname "$0")/.."

usage() { sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; }

say() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
warn() { printf '\033[33m   %s\033[0m\n' "$*"; }
die() { printf '\033[31mErreur : %s\033[0m\n' "$*" >&2; exit 1; }

# ---- arguments ----
DRY_RUN=0
ALL=0
SNAPSHOT=0
SNAPSHOT_FILE=scripts/rates.snapshot
LOOP=""
ARGS=()
while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --dry-run) DRY_RUN=1; ARGS+=("$1") ;;
    --all) ALL=1; ARGS+=("$1") ;;
    --snapshot) SNAPSHOT=1 ;;
    --loop)
      LOOP=${2:-}
      [[ "$LOOP" =~ ^[0-9]+$ ]] && [ "$LOOP" -ge 60 ] || die "--loop attend une période en secondes, au moins 60 (ex. --loop 3600)"
      shift ;;
    *) die "option inconnue : $1 (voir --help)" ;;
  esac
  shift
done

# ---- boucle : relance le script sans --loop ; un passage en échec n'arrête pas la boucle ----
if [ -n "$LOOP" ]; then
  [ "$SNAPSHOT" = 0 ] || die "--loop et --snapshot ne vont pas ensemble"
  while :; do
    echo "[$(date '+%F %T')] synchro des taux"
    "$0" ${ARGS[@]+"${ARGS[@]}"} || warn "passage en échec, nouvel essai dans $LOOP s"
    sleep "$LOOP"
  done
fi

# .env.testnet est facultatif : il ne sert qu'au repli sur POOL_ID.
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
[ -n "$POOL" ] || [ "$DRY_RUN" = 1 ] || [ "$SNAPSHOT" = 1 ] || die "pool introuvable : ni alias fx-pool ni POOL_ID dans .env.testnet (POOL=... pour le choisir)"

# Devise, stablebond Etherfuse (- : aucun), taux de repli en bps (- : aucun).
# Taux de repli = scripts/rates.snapshot (taux du pool déployé, chiffres de la démo), lus ci-dessous ;
# valeurs écrites ici = copie du snapshot du 25/10/2026, utilisées seulement si le fichier manque.
ROWS=("USD USTRY 360" "MXN CETES 555" "BRL TESOURO 1148" "ARS - 2200" "EUR EUROB 210")
if [ -f "$SNAPSHOT_FILE" ]; then
  for i in "${!ROWS[@]}"; do
    set -- ${ROWS[$i]}
    SNAP_BPS=$(awk -v c="$1" '$1 == c && $2 ~ /^[0-9]+$/ { print $2; exit }' "$SNAPSHOT_FILE")
    [ -z "$SNAP_BPS" ] || ROWS[$i]="$1 $2 $SNAP_BPS"
  done
fi
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
elif ! BODY=$(curl -fsS -m 10 --retry 2 --retry-delay 3 "$ETHERFUSE_URL" 2>&1); then
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

# ---- snapshot : taux figés pour setup-testnet.sh (aucun appel au pool) ----
if [ "$SNAPSHOT" = 1 ]; then
  [ "$N_EF" -gt 0 ] || die "aucun taux Etherfuse utilisable : snapshot non écrit (l'ancien est conservé)"
  NOW=$(date -u '+%Y-%m-%dT%H:%MZ')
  {
    echo "# Taux d'intérêt annuels en bps, figés pour scripts/setup-testnet.sh (qui n'appelle pas l'API)."
    echo "# Refaire exprès : ./scripts/sync-rates.sh --snapshot, puis relancer le setup et vérifier les chiffres de la démo."
    echo "# Source : $ETHERFUSE_URL (current_basis_points) ; en_dur = docs/todo-pricing.md §1.5."
    echo "# devise bps date source"
    for t in "${TABLE[@]}"; do
      set -- $t
      [ "$5" = - ] && continue
      if [ "$6" = etherfuse ]; then SRC="etherfuse:$2"; else SRC="en_dur"; fi
      echo "$1 $5 $NOW $SRC"
    done
  } > "$SNAPSHOT_FILE"
  say "Snapshot écrit : $SNAPSHOT_FILE"
  sed 's/^/   /' "$SNAPSHOT_FILE"
  [ "$N_FALLBACK" -eq 0 ] || warn "⚠️  $N_FALLBACK devise(s) en repli sur le taux en dur (voir ci-dessus)"
  exit 0
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
