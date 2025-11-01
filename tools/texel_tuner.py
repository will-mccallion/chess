# path: tools/texel_tuner_auto.py
import argparse
import io
import glob
import math
import os
import random
import sys
import time
from multiprocessing import Pool
from typing import Iterator, List, Optional, Tuple

import numpy as np
import chess

K_DEFAULT = 0.00575
LR_DEFAULT = 0.5
REG_DEFAULT = 1e-4
ITERS_DEFAULT = 100000  # high; use --max-seconds/early-stop to cap
WORKERS_DEFAULT = max(1, (os.cpu_count() or 1) - 4)
BATCH_DEFAULT = 20000
SAMPLE_DEFAULT = 0
ROTATE_MODE_DEFAULT = "random"  # or "roundrobin"
PATIENCE_DEFAULT = 3
LR_DECAY_DEFAULT = 0.9
MIN_LR_DEFAULT = 0.1
EARLY_STOP_DEFAULT = 10
MAX_SECONDS_DEFAULT = 0  # 0 = no wall-clock cap

PIECES = [chess.PAWN, chess.KNIGHT, chess.BISHOP, chess.ROOK, chess.QUEEN, chess.KING]
PIECE_NAMES = ["WP", "WN", "WB", "WR", "WQ", "WK"]
PHASE_VALUES = {chess.PAWN: 0, chess.KNIGHT: 1, chess.BISHOP: 1, chess.ROOK: 2, chess.QUEEN: 4, chess.KING: 0}
MAX_PHASE = 24

def initial_weights(dtype):
    w = {'mg': {}, 'eg': {}}
    w['mg']['WP'] = np.array([0,0,0,0,0,0,0,0,98,134,61,95,68,126,34,-11,-6,7,26,31,65,56,25,-20,-14,13,6,21,23,12,17,-23,-27,-2,-5,12,17,6,10,-25,-26,-4,-4,-10,3,3,33,-12,-35,-1,-20,-23,-15,24,38,-22,0,0,0,0,0,0,0,0], dtype=dtype)
    w['mg']['WN'] = np.array([-167,-89,-34,-49,61,-97,-15,-107,-73,-41,72,36,23,62,7,-17,-47,60,37,65,84,129,73,44,-9,17,19,53,37,69,18,22,-13,4,16,13,28,19,21,-8,-23,-9,12,10,19,17,25,-16,-29,-53,-12,-3,-1,18,-14,-19,-105,-21,-58,-33,-17,-28,-19,-23], dtype=dtype)
    w['mg']['WB'] = np.array([-29,4,-82,-37,-25,-42,7,-8,-26,16,-18,-13,30,59,18,-47,-16,37,43,40,35,50,37,-2,-4,5,19,50,37,37,7,-2,-6,13,13,26,34,12,10,4,0,15,15,15,14,27,18,10,4,15,16,0,7,21,33,1,-33,-3,-14,-21,-13,-12,-39,-21], dtype=dtype)
    w['mg']['WR'] = np.array([32,42,32,51,63,9,31,43,27,32,58,62,80,67,26,44,-5,19,26,36,17,45,61,16,-24,-11,7,26,24,35,-8,-20,-36,-26,-12,-1,9,-7,6,-23,-45,-25,-16,-17,3,0,-5,-33,-44,-16,-20,-9,-1,11,-6,-71,-19,-13,1,17,16,7,-37,-26], dtype=dtype)
    w['mg']['WQ'] = np.array([-28,0,29,12,59,44,43,45,-24,-39,-5,1,-16,57,28,54,-13,-17,7,8,29,56,47,57,-27,-27,-16,-16,-1,17,-2,1,-9,-26,-9,-10,-2,-4,3,-3,-14,2,-11,-2,-5,2,14,5,-35,-8,11,2,8,15,-3,1,-1,-18,-9,10,-15,-25,-31,-50], dtype=dtype)
    w['mg']['WK'] = np.array([-65,23,16,-15,-56,-34,2,13,29,-1,-20,-7,-8,-4,-38,-29,-9,24,2,-16,-20,6,22,-22,-17,-20,-12,-27,-30,-25,-14,-36,-49,-1,-27,-39,-46,-44,-33,-51,-14,-14,-22,-46,-44,-30,-15,-27,1,7,-8,-64,-43,-16,9,8,-15,36,12,-54,8,-28,24,14], dtype=dtype)
    w['eg']['WP'] = np.array([0,0,0,0,0,0,0,0,178,173,158,134,147,132,165,187,94,100,85,67,56,53,82,84,32,24,13,5,-2,4,17,17,13,9,-3,-7,-7,-8,3,-1,4,7,-6,1,0,-5,-1,-8,13,8,8,10,13,0,2,-7,0,0,0,0,0,0,0,0], dtype=dtype)
    w['eg']['WN'] = np.array([-58,-38,-13,-28,-31,-27,-63,-99,-25,-8,-25,-2,-9,-25,-24,-52,-24,-20,10,9,-1,-9,-19,-41,-17,3,22,22,22,11,8,-18,-18,-6,16,25,16,17,4,-18,-23,-3,-1,15,10,-3,-20,-22,-42,-20,-10,-5,-2,-20,-23,-44,-29,-51,-23,-15,-22,-18,-50,-64], dtype=dtype)
    w['eg']['WB'] = np.array([-14,-21,-11,-8,-7,-9,-17,-24,-8,7,-1,-2,3,-3,2,-15,-2,4,1,4,7,4,0,-5,-3,10,12,12,8,7,6,-3,-6,11,13,13,7,7,4,-3,-1,3,5,5,5,2,2,-8,-2,4,8,1,7,1,-3,-13,-29,-17,-4,-4,-5,-12,-12,-28], dtype=dtype)
    w['eg']['WR'] = np.array([13,10,18,15,12,12,8,5,11,13,13,11,-3,3,8,3,7,7,7,5,4,-3,-5,-3,4,3,13,1,2,1,-1,2,3,5,8,4,-5,-6,-8,-11,-4,0,-5,-1,-7,-12,-8,-16,-6,-6,0,2,-9,-9,-11,-3,-9,2,3,-1,-5,-13,4,-20], dtype=dtype)
    w['eg']['WQ'] = np.array([-9,22,22,27,27,19,10,20,-17,20,32,41,58,25,30,0,-20,6,9,49,47,35,19,9,3,22,24,45,57,40,57,36,-18,28,19,47,31,34,39,23,-16,-27,15,6,9,17,10,5,-22,-23,-30,-16,-16,-23,-36,-32,-33,-28,-22,-43,-5,-32,-20,-41], dtype=dtype)
    w['eg']['WK'] = np.array([-74,-35,-18,-18,-11,15,4,-17,-12,17,14,17,17,38,23,11,10,17,23,15,20,45,44,13,-8,22,24,27,26,33,26,3,-18,-4,21,24,27,23,9,-11,-19,-3,11,21,23,16,7,-9,-27,-11,4,13,15,-3,-11,-19,-53,-34,-21,-11,-28,-14,-24,-43], dtype=dtype)
    return w

def sigmoid(x: float, k: float) -> float:
    try:
        return 1.0 / (1.0 + math.exp(-k * x / 100.0))
    except OverflowError:
        return 1.0 if x > 0 else 0.0

def get_phase(board: chess.Board) -> int:
    phase = 0
    for pt in (chess.KNIGHT, chess.BISHOP, chess.ROOK, chess.QUEEN):
        phase += len(board.pieces(pt, chess.WHITE)) * PHASE_VALUES[pt]
        phase += len(board.pieces(pt, chess.BLACK)) * PHASE_VALUES[pt]
    return min(phase, MAX_PHASE)

def extract_features(board: chess.Board, nfeat: int, dtype) -> np.ndarray:
    f = np.zeros(nfeat, dtype=dtype)
    phase = get_phase(board)
    mg = phase / MAX_PHASE
    eg = (MAX_PHASE - phase) / MAX_PHASE
    for idx, pt in enumerate(PIECES):
        for sq in board.pieces(pt, chess.WHITE):
            r = chess.square_rank(sq)
            sym = sq if r < 4 else chess.square_mirror(sq)
            f[idx * 32 + sym] += mg
            f[len(PIECES) * 32 + idx * 32 + sym] += eg
        for sq in board.pieces(pt, chess.BLACK):
            msq = chess.square_mirror(sq)
            r = chess.square_rank(msq)
            sym = msq if r < 4 else chess.square_mirror(msq)
            f[idx * 32 + sym] -= mg
            f[len(PIECES) * 32 + idx * 32 + sym] -= eg
    return f

def flatten_weights(w: dict, dtype) -> np.ndarray:
    flat = []
    for phase in ('mg', 'eg'):
        for name in PIECE_NAMES:
            flat.extend(w[phase][name][:32])
    return np.asarray(flat, dtype=dtype)

def unflatten_weights(vec: np.ndarray, dtype) -> dict:
    res = {'mg': {}, 'eg': {}}
    p = 0
    for phase in ('mg', 'eg'):
        for name in PIECE_NAMES:
            full = np.zeros(64, dtype=dtype)
            half = vec[p:p+32]
            full[:32] = half
            for i in range(32):
                full[chess.square_mirror(i)] = half[i]
            res[phase][name] = full
            p += 32
    return res

def parse_epd_line(line: str) -> Optional[Tuple[chess.Board, float]]:
    s = line.strip()
    if not s or s.startswith("#"):
        return None
    parts = [p.strip() for p in s.split(";") if p.strip()]
    head = parts[0]
    hf = head.split()
    board = None
    if len(hf) >= 4:
        fen = f"{hf[0]} {hf[1]} {hf[2]} {hf[3]} 0 1"
        try:
            board = chess.Board(fen)
        except Exception:
            board = None
    if board is None:
        try:
            b = chess.Board()
            b.set_epd(head)
            board = b
        except Exception:
            return None
    outcome = None
    for op in parts[1:]:
        t = op.replace(":", " ").replace("=", " ")
        if "1-0" in t:
            outcome = 1.0; break
        if "0-1" in t:
            outcome = 0.0; break
        if "1/2-1/2" in t or "1/2" in t:
            outcome = 0.5; break
    if outcome is None:
        return None
    return board, outcome

def iter_lines(path: str) -> Iterator[str]:
    with open(path, "r", encoding="utf-8", errors="ignore") as f:
        for line in f:
            yield line

def iter_lines_slice_by_seek(path: str, n_lines: int, byte_offset: int) -> Iterator[str]:
    size = os.path.getsize(path)
    if size <= 0:
        return
    off = max(0, min(byte_offset, size - 1))
    with open(path, "rb", buffering=1024*1024) as fb:
        fb.seek(off)
        if off > 0:
            fb.readline()
        tb = io.TextIOWrapper(fb, encoding="utf-8", errors="ignore")
        for _ in range(n_lines):
            line = tb.readline()
            if not line:
                return
            yield line

def discover_shards(shards_dir: str) -> List[str]:
    files = sorted([p for p in glob.glob(os.path.join(shards_dir, "*")) if os.path.isfile(p)])
    if not files:
        raise FileNotFoundError(f"No shards found in: {shards_dir}")
    return files

def batched_lines(lines: Iterator[str], batch_size: int, limit_lines: int = 0) -> Iterator[List[str]]:
    batch: List[str] = []
    total = 0
    for line in lines:
        batch.append(line)
        total += 1
        if limit_lines and total >= limit_lines:
            if batch:
                yield batch
            return
        if len(batch) >= batch_size:
            yield batch
            batch = []
    if batch:
        yield batch

_W_FLAT = None
_K = None
_DTYPE = None
_N = None

def _init_worker(w_flat: np.ndarray, k: float, dtype_str: str):
    global _W_FLAT, _K, _DTYPE, _N
    _W_FLAT = w_flat
    _K = k
    _DTYPE = np.float32 if dtype_str == "float32" else np.float64
    _N = _W_FLAT.shape[0]

def _process_batch(lines: List[str]):
    n = _N
    dtype = _DTYPE
    A = np.zeros((n, n), dtype=dtype)
    b = np.zeros(n, dtype=dtype)
    sse = 0.0
    logloss = 0.0
    cnt = 0
    for line in lines:
        parsed = parse_epd_line(line)
        if parsed is None:
            continue
        board, outcome = parsed
        x = extract_features(board, n, dtype)
        tapered = float(np.dot(_W_FLAT, x))
        p = sigmoid(tapered, _K)
        err = outcome - p
        g = (_K / 100.0) * p * (1.0 - p)  # Texel weighting
        A += g * np.outer(x, x)
        b += err * x
        sse += err * err
        pp = min(max(p, 1e-12), 1 - 1e-12)
        logloss += -(outcome * math.log(pp) + (1.0 - outcome) * math.log(1.0 - pp))
        cnt += 1
    return A, b, sse, logloss, cnt

def format_for_rust(w_dict: dict, out) -> None:
    def dump_phase(phase_name, const):
        print(f"pub const {const}: [[i32; 64]; 13] = [", file=out)
        print("    [0; 64],", file=out)
        for i, name in enumerate(PIECE_NAMES):
            print(f"    // {name} ({i + 1})", file=out)
            print("    [", file=out)
            arr = w_dict[phase_name][name]
            for r in range(8):
                print("        " + ", ".join(str(int(round(x))) for x in arr[r*8:(r+1)*8]) + ",", file=out)
            print("    ],", file=out)
        for i, name in enumerate(PIECE_NAMES):
            bname = "B" + name[1]
            print(f"    // {bname} ({i + 7})", file=out)
            print("    [", file=out)
            arr = w_dict[phase_name][name]
            mir = np.flip(arr.reshape(8, 8), 0).flatten()
            for r in range(8):
                print("        " + ", ".join(str(int(round(-x))) for x in mir[r*8:(r+1)*8]) + ",", file=out)
            print("    ],", file=out)
        print("];\n", file=out)
    dump_phase('mg', 'MG_PST')
    dump_phase('eg', 'EG_PST')

def append_csv(csv_path: str, row: dict, header_written: set):
    os.makedirs(os.path.dirname(csv_path), exist_ok=True)
    write_header = False
    if csv_path not in header_written and not os.path.exists(csv_path):
        write_header = True
    with open(csv_path, "a") as f:
        if write_header:
            f.write(",".join(row.keys()) + "\n")
            header_written.add(csv_path)
        f.write(",".join(str(row[k]) for k in row.keys()) + "\n")

def train_parallel(
    data_path: str,
    shards_dir: Optional[str],
    rotate_mode: str,
    iters: int,
    sample: int,
    k: float,
    lr: float,
    reg: float,
    float32: bool,
    workers: int,
    batch_size: int,
    auto_shard_lines: int,
    log_pps: bool,
    checkpoint_path: Optional[str],
    weights_npy_path: Optional[str],
    csv_path: Optional[str],
    resume_path: Optional[str],
    patience: int,
    lr_decay: float,
    min_lr: float,
    early_stop: int,
    max_seconds: int,
):
    dtype = np.float32 if float32 else np.float64
    if resume_path and os.path.exists(resume_path):
        w_flat = np.load(resume_path).astype(dtype, copy=True)
        print(f"[resume] loaded weights from {resume_path} (shape {w_flat.shape})")
        weights = unflatten_weights(w_flat, dtype)
    else:
        weights = initial_weights(dtype)
        w_flat = flatten_weights(weights, dtype)

    n = w_flat.shape[0]
    if n != 384:
        print(f"FATAL: expected 384 params, got {n}."); sys.exit(1)

    shard_files: List[str] = []
    if shards_dir:
        shard_files = discover_shards(shards_dir)

    rr_idx = 0
    file_size = os.path.getsize(data_path) if data_path and os.path.isfile(data_path) else 0
    start_wall = time.time()
    best_ll = float("inf")
    stale = 0
    header_written = set()

    for it in range(1, iters + 1):
        # wall-clock guard
        if max_seconds and (time.time() - start_wall) >= max_seconds:
            print(f"[time] reached max-seconds={max_seconds}, stopping after completed iters.")
            break

        # choose data source
        if shard_files:
            if rotate_mode == "roundrobin":
                shard = shard_files[rr_idx % len(shard_files)]; rr_idx += 1
            else:
                shard = random.choice(shard_files)
            line_iter = iter_lines(shard)
            limit_lines = sample if sample > 0 else 0
            src_desc = f"shard={os.path.basename(shard)}"
        elif auto_shard_lines > 0 and file_size > 0:
            if rotate_mode == "roundrobin":
                step = max(1, file_size // max(1, iters))
                byte_offset = ((it - 1) * step) % file_size
            else:
                byte_offset = random.randrange(0, file_size)
            line_iter = iter_lines_slice_by_seek(data_path, auto_shard_lines, byte_offset)
            limit_lines = 0
            src_desc = f"auto-shard {auto_shard_lines} lines @ byte {byte_offset}"
        else:
            line_iter = iter_lines(data_path)
            limit_lines = sample if sample > 0 else 0
            src_desc = os.path.basename(data_path)

        print(f"\n--- Iteration {it}/{iters} ---")
        if log_pps:
            print(f"  source: {src_desc}")

        A = np.identity(n, dtype=dtype) * reg
        b = np.zeros(n, dtype=dtype)
        total_sse = 0.0
        total_logloss = 0.0
        total_cnt = 0
        t0 = time.time()
        dtype_str = "float32" if float32 else "float64"

        with Pool(processes=workers, initializer=_init_worker, initargs=(w_flat, k, dtype_str)) as pool:
            for A_part, b_part, sse, lls, cnt in pool.imap_unordered(
                _process_batch, batched_lines(line_iter, batch_size, limit_lines), chunksize=1
            ):
                if cnt == 0:
                    continue
                A += A_part
                b += b_part
                total_sse += sse
                total_logloss += lls
                total_cnt += cnt
                if log_pps and (total_cnt % (batch_size * max(1, workers)) == 0):
                    dt = time.time() - t0
                    pps = total_cnt / dt if dt > 0 else 0.0
                    print(f"  {total_cnt:,} positions | {pps:,.0f} pos/s")

        if total_cnt == 0:
            print("ERROR: no valid positions parsed."); sys.exit(1)

        try:
            update, *_ = np.linalg.lstsq(A, b, rcond=None)
        except np.linalg.LinAlgError:
            print("  WARNING: linear solve failed; skipping update")
            update = np.zeros_like(w_flat)

        u_l2 = float(np.linalg.norm(update))
        u_inf = float(np.max(np.abs(update))) if update.size else 0.0
        w_flat = w_flat + lr * update
        weights = unflatten_weights(w_flat, dtype)

        avg_mse = float(total_sse / total_cnt)
        avg_ll = float(total_logloss / total_cnt)
        dt = time.time() - t0
        pps = total_cnt / dt if dt > 0 else 0.0

        print(f"  positions={total_cnt:,}  solving {n}x{n} ...")
        print(f"  avg_mse={avg_mse:.6f}  avg_logloss={avg_ll:.6f}  |Δ|2={u_l2:.4f}  max|Δ|={u_inf:.4f}  lr={lr:.4f}  time={dt:.2f}s  throughput={pps:,.0f} pos/s")

        # CSV log
        if csv_path:
            row = {
                "iter": it,
                "positions": total_cnt,
                "avg_mse": f"{avg_mse:.6f}",
                "avg_logloss": f"{avg_ll:.6f}",
                "delta_l2": f"{u_l2:.6f}",
                "delta_inf": f"{u_inf:.6f}",
                "lr": f"{lr:.6f}",
                "time_sec": f"{dt:.3f}",
                "pps": f"{pps:.1f}",
                "source": f"\"{src_desc}\"",
            }
            append_csv(csv_path, row, header_written)

        # Checkpoint on improvement
        improved = avg_ll + 1e-4 < best_ll
        if improved:
            best_ll = avg_ll
            stale = 0
            if checkpoint_path:
                os.makedirs(os.path.dirname(checkpoint_path), exist_ok=True)
                with open(checkpoint_path, "w") as f:
                    format_for_rust(weights, f)
                print(f"  [checkpoint] new best avg_logloss={best_ll:.6f} -> {checkpoint_path}")
            if weights_npy_path:
                os.makedirs(os.path.dirname(weights_npy_path), exist_ok=True)
                np.save(weights_npy_path, w_flat)
        else:
            stale += 1
            if stale >= patience and lr > min_lr:
                lr = max(min_lr, lr * lr_decay)
                stale = 0
                print(f"  [lr-decay] lr -> {lr:.4f}")
            if stale >= early_stop:
                print(f"  [early-stop] no improvement for {early_stop} checks, stopping.")
                break

        # wall-clock cap (respect after full iter)
        if max_seconds and (time.time() - start_wall) >= max_seconds:
            print(f"[time] reached max-seconds={max_seconds}, stopping after completed iter.")
            break

    # Final export to stdout
    print("\n--- Done ---")
    format_for_rust(unflatten_weights(w_flat, dtype), sys.stdout)

def main():
    ap = argparse.ArgumentParser(description="Texel tuner (parallel) with auto-shard, patience/LR-decay, checkpoints, CSV logs")
    ap.add_argument("--data", default="positions/positions.epd")
    ap.add_argument("--shards-dir", default=None)
    ap.add_argument("--rotate-mode", choices=["random", "roundrobin"], default=ROTATE_MODE_DEFAULT)
    ap.add_argument("--iters", type=int, default=ITERS_DEFAULT)
    ap.add_argument("--sample", type=int, default=SAMPLE_DEFAULT, help="max lines per iter (0=full stream)")
    ap.add_argument("--auto-shard", type=int, default=0, metavar="N", help="if set, pick a new slice of N lines each iter via byte-seek")
    ap.add_argument("--k", type=float, default=K_DEFAULT)
    ap.add_argument("--lr", type=float, default=LR_DEFAULT)
    ap.add_argument("--reg", type=float, default=REG_DEFAULT)
    ap.add_argument("--float32", action="store_true", help="use float32")
    ap.add_argument("--workers", type=int, default=WORKERS_DEFAULT)
    ap.add_argument("--batch", type=int, default=BATCH_DEFAULT, help="lines per worker task")
    ap.add_argument("--log-pps", action="store_true")
    ap.add_argument("--checkpoint", default="runs/best_pst.rs")
    ap.add_argument("--weights-npy", default="runs/best_weights.npy")
    ap.add_argument("--csv", default="runs/metrics.csv")
    ap.add_argument("--resume", default=None, help="path to .npy weights to resume from")
    ap.add_argument("--patience", type=int, default=PATIENCE_DEFAULT)
    ap.add_argument("--lr-decay", type=float, default=LR_DECAY_DEFAULT)
    ap.add_argument("--min-lr", type=float, default=MIN_LR_DEFAULT)
    ap.add_argument("--early-stop", type=int, default=EARLY_STOP_DEFAULT)
    ap.add_argument("--max-seconds", type=int, default=MAX_SECONDS_DEFAULT, help="stop after this many seconds (checked per iter)")
    args = ap.parse_args()

    train_parallel(
        data_path=args.data,
        shards_dir=args.shards_dir,
        rotate_mode=args.rotate_mode,
        iters=args.iters,
        sample=args.sample,
        k=args.k,
        lr=args.lr,
        reg=args.reg,
        float32=args.float32,
        workers=max(1, args.workers),
        batch_size=max(1000, args.batch),
        auto_shard_lines=max(0, args.auto_shard),
        log_pps=args.log_pps,
        checkpoint_path=args.checkpoint,
        weights_npy_path=args.weights_npy,
        csv_path=args.csv,
        resume_path=args.resume,
        patience=max(1, args.patience),
        lr_decay=args.lr_decay,
        min_lr=args.min_lr,
        early_stop=max(1, args.early_stop),
        max_seconds=max(0, args.max_seconds),
    )

if __name__ == "__main__":
    main()

