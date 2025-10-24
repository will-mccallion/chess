#!/usr/bin/env bash
# run_match.sh — run Engine A vs Engine B (both UCI), powered by cutechess-cli
# Examples:
#   ./run_match.sh --a ./versions/chess_v1 --b ./versions/chess_v2
#   ./run_match.sh --a ./v1 --b ./v2 --games 100 --concurrency 8 --st 0.5 --pgn v1v2.pgn
#   ./run_match.sh --a ./v1 --b ./v2 --a-opt Hash=16 --b-opt Threads=2 --openings book.epd --plies 16
#   ./run_match.sh --a ./v1 --b ./v2 --ponder
#
# UCI options are passed with --a-opt key=value (repeatable) and --b-opt key=value (repeatable).

set -euo pipefail

# ---------- Defaults ----------
ENGINE_A_BIN="./target/release/chess"
ENGINE_A_ARG="uci"
ENGINE_A_NAME="New"  # will auto-default to basename of ENGINE_A_BIN

ENGINE_B_BIN="./versions/chess_v3_4"
ENGINE_B_ARG="uci"
ENGINE_B_NAME="v3.4"  # will auto-default to basename of ENGINE_B_BIN

# Match/time defaults
ST_SECONDS="1.0"        # per-move time
TIME_MARGIN="5000"       # ms
GAMES="20"
CONCURRENCY="4"
PGN_OUT="match.pgn"
PONDER=false

# Optional openings
OPENINGS_FILE=""
OPENINGS_FORMAT="epd"   # epd | pgn
OPENINGS_ORDER="random" # random | sequential
OPENINGS_PLIES=""       # e.g. 16 (plies to play from the opening)

# Extra args to cutechess after `--`
EXTRA_ARGS=()
A_OPTS=()  # per-engine UCI options for A, as option.<Key>=<Val>
B_OPTS=()  # per-engine UCI options for B, as option.<Key>=<Val>

usage() {
  cat <<EOF
Usage: $0 [options] [-- extra_cutechess_args...]

Engine A:
  --a <path>                Path to engine A binary (default: $ENGINE_A_BIN)
  --a-arg <arg>             Argument to start UCI loop for A (default: $ENGINE_A_ARG)
  --a-name <name>           Engine A display name (default: basename of binary)
  --a-opt K=V               UCI option for A (repeatable), e.g. --a-opt Hash=16

Engine B:
  --b <path>                Path to engine B binary (default: $ENGINE_B_BIN)
  --b-arg <arg>             Argument to start UCI loop for B (default: $ENGINE_B_ARG)
  --b-name <name>           Engine B display name (default: basename of binary)
  --b-opt K=V               UCI option for B (repeatable), e.g. --b-opt Threads=2

Match/Time:
  --st <sec>                Per-move time (default: $ST_SECONDS)
  --timemargin <ms>         Time margin (default: $TIME_MARGIN)
  --games <n>               Number of games (default: $GAMES)
  --concurrency <n>         Parallel games (default: $CONCURRENCY)
  --pgn <file>              PGN output (default: $PGN_OUT)
  --ponder                  Enable pondering for both engines

Openings (optional):
  --openings <file>         EPD or PGN opening file for starting positions
  --format <epd|pgn>        Format for --openings (default: $OPENINGS_FORMAT)
  --order <random|sequential>  Order for openings (default: $OPENINGS_ORDER)
  --plies <n>               Number of plies from opening to play out (optional)

Other:
  -h, --help                Show this help

Anything after '--' is appended raw to the cutechess-cli command.
EOF
}

# ---------- Parse args ----------
while [[ $# -gt 0 ]]; do
  case "$1" in
    --a) ENGINE_A_BIN="$2"; shift 2;;
    --a-arg) ENGINE_A_ARG="$2"; shift 2;;
    --a-name) ENGINE_A_NAME="$2"; shift 2;;
    --a-opt)
      [[ "$2" == *=* ]] || { echo "Expected K=V after --a-opt"; exit 1; }
      A_OPTS+=("option.${2%=*}=${2#*=}")
      shift 2;;
    --b) ENGINE_B_BIN="$2"; shift 2;;
    --b-arg) ENGINE_B_ARG="$2"; shift 2;;
    --b-name) ENGINE_B_NAME="$2"; shift 2;;
    --b-opt)
      [[ "$2" == *=* ]] || { echo "Expected K=V after --b-opt"; exit 1; }
      B_OPTS+=("option.${2%=*}=${2#*=}")
      shift 2;;
    --st) ST_SECONDS="$2"; shift 2;;
    --timemargin) TIME_MARGIN="$2"; shift 2;;
    --games) GAMES="$2"; shift 2;;
    --concurrency) CONCURRENCY="$2"; shift 2;;
    --pgn) PGN_OUT="$2"; shift 2;;
    --ponder) PONDER=true; shift;;
    --openings) OPENINGS_FILE="$2"; shift 2;;
    --format) OPENINGS_FORMAT="$2"; shift 2;;
    --order) OPENINGS_ORDER="$2"; shift 2;;
    --plies) OPENINGS_PLIES="$2"; shift 2;;
    -h|--help) usage; exit 0;;
    --) shift; EXTRA_ARGS=("$@"); break;;
    *) echo "Unknown option: $1"; usage; exit 1;;
  esac
done

# ---------- Preflight ----------
command -v cutechess-cli >/dev/null 2>&1 || { echo "Error: cutechess-cli not found"; exit 1; }
[[ -x "$ENGINE_A_BIN" ]] || { echo "Error: engine A not executable at '$ENGINE_A_BIN'"; exit 1; }
[[ -x "$ENGINE_B_BIN" ]] || { echo "Error: engine B not executable at '$ENGINE_B_BIN'"; exit 1; }

# Auto names if not provided
if [[ -z "$ENGINE_A_NAME" ]]; then ENGINE_A_NAME="$(basename "$ENGINE_A_BIN")"; fi
if [[ -z "$ENGINE_B_NAME" ]]; then ENGINE_B_NAME="$(basename "$ENGINE_B_BIN")"; fi

# ---------- Build engine specs ----------
A_SPEC=( cmd="$ENGINE_A_BIN" name="$ENGINE_A_NAME" proto=uci )
B_SPEC=( cmd="$ENGINE_B_BIN" name="$ENGINE_B_NAME" proto=uci )

# Add UCI loop arg if set
[[ -n "$ENGINE_A_ARG" ]] && A_SPEC+=( arg="$ENGINE_A_ARG" )
[[ -n "$ENGINE_B_ARG" ]] && B_SPEC+=( arg="$ENGINE_B_ARG" )

# Enable pondering if requested
if [[ "$PONDER" = true ]]; then
  A_OPTS+=("option.Ponder=true")
  B_OPTS+=("option.Ponder=true")
fi

# Add per-engine UCI options
if [[ ${#A_OPTS[@]} -gt 0 ]]; then A_SPEC+=( "${A_OPTS[@]}" ); fi
if [[ ${#B_OPTS[@]} -gt 0 ]]; then B_SPEC+=( "${B_OPTS[@]}" ); fi

# ---------- Assemble command ----------
CMD=( cutechess-cli
  -engine "${A_SPEC[@]}"
  -engine "${B_SPEC[@]}"
  -each st="$ST_SECONDS" timemargin="$TIME_MARGIN"
  -games "$GAMES" -concurrency "$CONCURRENCY"
  -pgnout "$PGN_OUT"
)

# Openings (optional)
if [[ -n "$OPENINGS_FILE" ]]; then
  [[ -f "$OPENINGS_FILE" ]] || { echo "Error: openings file '$OPENINGS_FILE' not found"; exit 1; }
  CMD+=( -openings file="$OPENINGS_FILE" format="$OPENINGS_FORMAT" order="$ORDER" )
  if [[ -n "$OPENINGS_PLIES" ]]; then
    CMD+=( plies="$OPENINGS_PLIES" )
  fi
fi

# Extra cutechess args
if [[ ${#EXTRA_ARGS[@]} -gt 0 ]]; then
  CMD+=( "${EXTRA_ARGS[@]}" )
fi

echo "Running:"
printf '  %q ' "${CMD[@]}"; echo; echo
exec "${CMD[@]}"
