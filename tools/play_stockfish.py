#!/usr/bin/env python3
# file: play_stockfish.py
"""
cutechess-cli runner with configurable Stockfish ELO (default 2500).

Examples:
  python play_stockfish.py                          # ELO 2500
  python play_stockfish.py --elo 2200               # cap SF at 2200
  python play_stockfish.py --engine-a ./myengine    # custom engine A
  python play_stockfish.py --tc 5+0.05 --rounds 50

Requires: cutechess-cli, your engine binary, stockfish binary.
"""

from __future__ import annotations

import argparse
import os
import re
import shlex
import shutil
import sys
from dataclasses import dataclass, field
from datetime import datetime
from subprocess import PIPE, Popen
from typing import Dict, List, Optional, Tuple

try:
    from rich.console import Console
    from rich.live import Live
    from rich.panel import Panel
    from rich.table import Table
    RICH = True
    console = Console()
except Exception:
    RICH = False
    console = None  # type: ignore


@dataclass
class EngineOptions:
    threads: int = 1
    hash_mb: int = 128
    extra: Dict[str, str] = field(default_factory=dict)


@dataclass
class DrawRule:
    movenumber: int = 40
    movecount: int = 25
    score: int = 0


@dataclass
class ResignRule:
    movecount: int = 25
    score: int = 1000


@dataclass
class Sprt:
    elo0: int = 0
    elo1: int = 0
    alpha: float = 0.05
    beta: float = 0.05


@dataclass
class Config:
    # Binaries
    cutechess: str = shutil.which("cutechess-cli") or "cutechess-cli"
    engine_a_cmd: str = "./target/release/chess"
    engine_a_name: str = "chess"
    engine_a_proto: str = "uci"
    engine_a_options: EngineOptions = field(default_factory=EngineOptions)

    stockfish_cmd: str = "/bin/stockfish"
    stockfish_base_name: str = "Stockfish"
    stockfish_proto: str = "uci"
    stockfish_options: EngineOptions = field(default_factory=EngineOptions)

    # Match settings
    time_control: str = "15+0.15"
    rounds: int = 100
    concurrency: int = 8
    pgn_out: str = "results_frc.pgn"
    sprt: Sprt = field(default_factory=Sprt)
    draw_rule: DrawRule = field(default_factory=DrawRule)
    resign_rule: ResignRule = field(default_factory=ResignRule)

    # Misc
    log_file: Optional[str] = None
    variant: Optional[str] = None


def which_or_raise(cmd: str) -> str:
    """
    Fail-fast if a required binary is missing.
    """
    if os.path.sep in cmd:
        if os.path.isfile(cmd) and os.access(cmd, os.X_OK):
            return cmd
        raise FileNotFoundError(f"Executable not found or not executable: {cmd}")
    path = shutil.which(cmd)
    if not path:
        raise FileNotFoundError(f"Required binary not found in PATH: {cmd}")
    return path


def build_engine_args(cmd: str, name: str, proto: str, opts: EngineOptions, extra_kv: Dict[str, str]) -> List[str]:
    args = [
        "-engine",
        f"cmd={cmd}",
        f"name={name}",
        f"proto={proto}",
        f"option.Threads={opts.threads}",
        f"option.Hash={opts.hash_mb}",
    ]
    for k, v in {**opts.extra, **extra_kv}.items():
        args.append(f"option.{k}={v}")
    return args


def build_cutechess_command(cfg: Config, stockfish_elo: int) -> List[str]:
    sf_name = f"{cfg.stockfish_base_name} {stockfish_elo}"
    sf_extras = {
        "UCI_LimitStrength": "true",
        "UCI_Elo": str(stockfish_elo),
    }

    cmd: List[str] = [cfg.cutechess]

    if cfg.variant:
        cmd += ["-variant", cfg.variant]

    # Engine A (your engine)
    cmd += build_engine_args(
        cmd=cfg.engine_a_cmd,
        name=cfg.engine_a_name,
        proto=cfg.engine_a_proto,
        opts=cfg.engine_a_options,
        extra_kv={}
    )

    # Engine B (Stockfish)
    cmd += build_engine_args(
        cmd=cfg.stockfish_cmd,
        name=sf_name,
        proto=cfg.stockfish_proto,
        opts=cfg.stockfish_options,
        extra_kv=sf_extras
    )

    # Shared match settings
    cmd += [
        "-each", f"tc={cfg.time_control}",
        "-rounds", str(cfg.rounds),
        "-concurrency", str(cfg.concurrency),
        "-pgnout", cfg.pgn_out,
        "-sprt",
        f"elo0={cfg.sprt.elo0}", f"elo1={cfg.sprt.elo1}",
        f"alpha={cfg.sprt.alpha}", f"beta={cfg.sprt.beta}",
        "-draw",
        f"movenumber={cfg.draw_rule.movenumber}",
        f"movecount={cfg.draw_rule.movecount}",
        f"score={cfg.draw_rule.score}",
        "-resign",
        f"movecount={cfg.resign_rule.movecount}",
        f"score={cfg.resign_rule.score}",
    ]
    return cmd

RE_SCORE = re.compile(r"^Score of .*?:\s*([+\-]?\d+(?:\.\d+)?)\s*-\s*([+\-]?\d+(?:\.\d+)?)\s*-\s*([+\-]?\d+(?:\.\d+)?).*", re.I)
RE_SPRT = re.compile(r"SPRT:\s*(.*)", re.I)
RE_RESULT = re.compile(r"Finished game\s+#(?P<n>\d+).*?(?P<res>1-0|0-1|1/2-1/2)", re.I)

@dataclass
class RunStats:
    games: int = 0
    w: int = 0
    l: int = 0
    d: int = 0
    score_line: Optional[Tuple[float, float, float]] = None
    sprt_line: Optional[str] = None

    def record(self, line: str) -> None:
        m = RE_RESULT.search(line)
        if m:
            self.games = max(self.games, int(m.group("n")))
            res = m.group("res")
            if res == "1-0":
                self.w += 1
            elif res == "0-1":
                self.l += 1
            else:
                self.d += 1
        m2 = RE_SCORE.search(line)
        if m2:
            self.score_line = (float(m2.group(1)), float(m2.group(2)), float(m2.group(3)))
        m3 = RE_SPRT.search(line)
        if m3:
            self.sprt_line = m3.group(1).strip()


def ensure_log_path(path: Optional[str]) -> str:
    return path or f"cutechess_run_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log"


def run(cfg: Config, elo: int) -> int:
    cfg.cutechess = which_or_raise(cfg.cutechess)
    which_or_raise(cfg.engine_a_cmd)
    which_or_raise(cfg.stockfish_cmd)

    cmd = build_cutechess_command(cfg, elo)
    log_path = ensure_log_path(cfg.log_file)
    stats = RunStats()

    if RICH:
        console.print(Panel.fit("Starting cutechess-cli"))
        console.print("Command:\n" + " ".join(shlex.quote(p) for p in cmd))
        console.print(f"Log: {log_path}\n")
    else:
        print("Command:", " ".join(shlex.quote(p) for p in cmd))
        print(f"Log: {log_path}\n")

    with open(log_path, "w", encoding="utf-8") as log, Popen(cmd, stdout=PIPE, stderr=PIPE, text=True, bufsize=1) as proc:  # noqa: SIM115
        try:
            if RICH:
                def render() -> Panel:
                    table = Table(expand=True)
                    table.add_column("Metric")
                    table.add_column("Value", justify="right")
                    table.add_row("Games", str(stats.games))
                    table.add_row("W / L / D", f"{stats.w} / {stats.l} / {stats.d}")
                    table.add_row("Score line", f"{stats.score_line}" if stats.score_line else "-")
                    table.add_row("SPRT", stats.sprt_line or "-")
                    return Panel(table, title="Match Progress")

                with Live(render(), console=console, refresh_per_second=4) as live:
                    assert proc.stdout is not None
                    for line in proc.stdout:
                        log.write(line)
                        stats.record(line)
                        live.update(render())
            else:
                assert proc.stdout is not None
                for line in proc.stdout:
                    sys.stdout.write(line)
                    log.write(line)
                    stats.record(line)

            # Drain stderr at the end (why: keep stdout responsive)
            if proc.stderr:
                err = proc.stderr.read()
                if err:
                    with open(log_path, "a", encoding="utf-8") as log2:
                        log2.write("\n[STDERR]\n" + err)

        finally:
            proc.wait()
            rc = proc.returncode

    # Summary
    if RICH:
        console.rule("[bold]Summary")
    print("\n=== Summary ===")
    print(f"Games: {stats.games}  |  W/L/D: {stats.w}/{stats.l}/{stats.d}")
    if stats.score_line:
        a, b, c = stats.score_line
        print(f"Score line: {a} - {b} - {c}")
    if stats.sprt_line:
        print(f"SPRT: {stats.sprt_line}")
    print("PGN:", cfg.pgn_out)
    print("Log:", log_path)
    print("Command:", " ".join(shlex.quote(p) for p in cmd))
    return rc


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Run cutechess-cli with configurable Stockfish ELO.")
    p.add_argument("--elo", type=int, default=int(os.environ.get("SF_ELO", "2500")),
                   help="Stockfish UCI_Elo (default: 2500 or SF_ELO env).")
    p.add_argument("--tc", "--time-control", dest="tc", type=str, default=None,
                   help="Time control like 15+0.15")
    p.add_argument("--rounds", type=int, default=None, help="Number of rounds")
    p.add_argument("--concurrency", type=int, default=None, help="Concurrent games")
    p.add_argument("--pgnout", "--pgn-out", dest="pgnout", type=str, default=None,
                   help="PGN output file")
    # Accept both hyphen and underscore for the same dests (fixes your error).
    p.add_argument("--engine-a", "--engine_a", dest="engine_a", type=str, default=None,
                   help="Path to engine A (your engine)")
    p.add_argument("--stockfish", type=str, default=None, help="Path to Stockfish binary")
    p.add_argument("--threads-a", "--threads_a", dest="threads_a", type=int, default=None,
                   help="Threads for engine A")
    p.add_argument("--threads-sf", "--threads_sf", dest="threads_sf", type=int, default=None,
                   help="Threads for Stockfish")
    p.add_argument("--hash-a", "--hash_a", dest="hash_a", type=int, default=None,
                   help="Hash MB for engine A")
    p.add_argument("--hash-sf", "--hash_sf", dest="hash_sf", type=int, default=None,
                   help="Hash MB for Stockfish")
    p.add_argument("--log", type=str, default=None, help="Log file path")
    p.add_argument("--variant", type=str, default=None, help='cutechess variant (e.g. "fischerandom")')
    return p.parse_args()


def main() -> int:
    args = parse_args()

    cfg = Config()
    if args.tc: cfg.time_control = args.tc
    if args.rounds is not None: cfg.rounds = args.rounds
    if args.concurrency is not None: cfg.concurrency = args.concurrency
    if args.pgnout: cfg.pgn_out = args.pgnout
    if args.engine_a: cfg.engine_a_cmd = args.engine_a
    if args.stockfish: cfg.stockfish_cmd = args.stockfish
    if args.threads_a is not None: cfg.engine_a_options.threads = args.threads_a
    if args.threads_sf is not None: cfg.stockfish_options.threads = args.threads_sf
    if args.hash_a is not None: cfg.engine_a_options.hash_mb = args.hash_a
    if args.hash_sf is not None: cfg.stockfish_options.hash_mb = args.hash_sf
    if args.log: cfg.log_file = args.log
    if args.variant: cfg.variant = args.variant

    elo = max(0, args.elo)
    return run(cfg, elo)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except FileNotFoundError as e:
        print(f"Error: {e}", file=sys.stderr)  # why: give a clear missing-binary message
        sys.exit(127)
    except KeyboardInterrupt:
        print("\nAborted by user.", file=sys.stderr)
        sys.exit(130)
