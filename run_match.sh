#!/usr/bin/env bash
# run_match.sh — run Rusty vs Stockfish with sane defaults
# Default command (unless you override with flags) is:
# cutechess-cli \
#   -engine cmd=./target/release/chess arg=uci name=Rusty proto=uci \
#   -engine cmd=/usr/bin/stockfish name=Stockfish proto=uci \
#            option.UCI_LimitStrength=true option.UCI_Elo=1350 option.Threads=1 option.Hash=16 \
#   -each st=1.0 timemargin=200 \
#   -games 20 -concurrency 4 -pgnout match.pgn

set -euo pipefail

# --- Defaults (match your requested command) ---
ENGINE_BIN="./versions/chess_v2"
ENGINE_ARG="uci"
ENGINE_NAME="version 2"

SF_BIN="/usr/bin/stockfish"
SF_NAME="Stockfish"
SF_LIMIT_STRENGTH="true"
SF_ELO="1350"
SF_THREADS="1"
SF_HASH="16"

ST_SECONDS="1.0"
TIME_MARGIN="200"
GAMES="20"
CONCURRENCY="4"
PGN_OUT="match.pgn"

# --- Minimal flags to override the basics ---
usage() {
  cat <<EOF
Usage: $0 [options] [-- extra_cutechess_args...]

Options:
  --engine <path>            Path to your engine (default: ${ENGINE_BIN})
  --engine-arg <arg>         Arg to start UCI loop (default: ${ENGINE_ARG})
  --stockfish <path>         Path to Stockfish (default: ${SF_BIN})
  --elo <n>                  Stockfish UCI_Elo (default: ${SF_ELO}; set to 0 to disable limit strength)
  --threads <n>              Stockfish Threads (default: ${SF_THREADS})
  --hash <MB>                Stockfish Hash (default: ${SF_HASH})
  --st <sec>                 Per-move time (default: ${ST_SECONDS})
  --timemargin <ms>          Time margin (default: ${TIME_MARGIN})
  --games <n>                Number of games (default: ${GAMES})
  --concurrency <n>          Concurrency (default: ${CONCURRENCY})
  --pgn <file>               PGN output file (default: ${PGN_OUT})
  -h, --help                 This help

Anything after '--' is appended raw to the cutechess-cli command.
EOF
}

EXTRA_ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --engine) ENGINE_BIN="$2"; shift 2;;
    --engine-arg) ENGINE_ARG="$2"; shift 2;;
    --stockfish) SF_BIN="$2"; shift 2;;
    --elo) SF_ELO="$2"; shift 2;;
    --threads) SF_THREADS="$2"; shift 2;;
    --hash) SF_HASH="$2"; shift 2;;
    --st) ST_SECONDS="$2"; shift 2;;
    --timemargin) TIME_MARGIN="$2"; shift 2;;
    --games) GAMES="$2"; shift 2;;
    --concurrency) CONCURRENCY="$2"; shift 2;;
    --pgn) PGN_OUT="$2"; shift 2;;
    -h|--help) usage; exit 0;;
    --) shift; EXTRA_ARGS=("$@"); break;;
    *) echo "Unknown option: $1"; usage; exit 1;;
  esac
done

# Preflight
command -v cutechess-cli >/dev/null 2>&1 || { echo "Error: cutechess-cli not found"; exit 1; }
[[ -x "$ENGINE_BIN" ]] || { echo "Error: engine not executable at '$ENGINE_BIN'"; exit 1; }
[[ -x "$SF_BIN" ]] || { echo "Error: stockfish not executable at '$SF_BIN'"; exit 1; }

# Build Stockfish options
SF_SPEC=( cmd="$SF_BIN" name="$SF_NAME" proto=uci option.Threads="$SF_THREADS" option.Hash="$SF_HASH" )
if [[ "$SF_ELO" != "0" ]]; then
  SF_SPEC+=( option.UCI_LimitStrength="$SF_LIMIT_STRENGTH" option.UCI_Elo="$SF_ELO" )
fi

# Assemble command (matches your default)
CMD=(
  cutechess-cli
  -engine cmd="$ENGINE_BIN" arg="$ENGINE_ARG" name="$ENGINE_NAME" proto=uci
  -engine "${SF_SPEC[@]}"
  -each st="$ST_SECONDS" timemargin="$TIME_MARGIN"
  -games "$GAMES" -concurrency "$CONCURRENCY" -pgnout "$PGN_OUT"
  "${EXTRA_ARGS[@]}"
)

echo "Running:"
printf '  %q ' "${CMD[@]}"; echo; echo
exec "${CMD[@]}"

