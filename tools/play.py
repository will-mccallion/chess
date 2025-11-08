# filepath: uci_arena_dark.py
# Requirements: pip install PySide6 python-chess
import sys
import os
import json
import random
import math
import csv
import time
import io
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

from PySide6 import QtCore, QtGui, QtWidgets
import chess
import chess.engine
import chess.pgn

APP_TITLE = "Code Copilot UCI Arena (Dark)"
VERSION = "1.4.6" # feat: Added move navigation buttons for game review

APP_DIR = os.path.abspath(os.path.dirname(__file__))
PRESETS_PATH = os.path.join(APP_DIR, "engines.json")

DARK_QSS = """
* { background-color: #181a1b; color: #e8e6e3; selection-background-color: #2a2d2e; selection-color: #e8e6e3; }
QToolTip { color: #e8e6e3; background-color: #2a2d2e; border: 1px solid #3a3d3e; }
QLineEdit, QPlainTextEdit, QTextEdit, QSpinBox, QDoubleSpinBox, QComboBox, QTableWidget, QTreeWidget, QListWidget {
    background-color: #0f1112; border: 1px solid #3a3d3e; border-radius: 6px; padding: 6px;
}
QTableWidget::item { padding: 6px; }
QPushButton { background-color: #232627; border: 1px solid #3a3d3e; border-radius: 8px; padding: 8px 12px; }
QPushButton:hover { background-color: #2a2d2e; }
QPushButton:pressed { background-color: #202324; }
QPushButton[enabled="false"] { background-color: #1c1f20; color: #5a5d5e; }
QProgressBar { border: 1px solid #3a3d3e; border-radius: 6px; background: #0f1112; text-align: center; }
QProgressBar::chunk { background-color: #3b82f6; }
QTabWidget::pane { border-top: 1px solid #3a3d3e; }
QTabBar::tab { background: #232627; padding: 10px 14px; border: 1px solid #3a3d3e; border-bottom: none; border-top-left-radius: 8px; border-top-right-radius: 8px; margin-right: 4px; }
QTabBar::tab:selected { background: #181a1b; }
QHeaderView::section { background: #232627; border: 1px solid #3a3d3e; padding: 6px; }
QGroupBox { border: 1px solid #3a3d3e; border-radius: 8px; margin-top: 10px; }
QGroupBox::title { subcontrol-origin: margin; left: 10px; padding: 0 3px 0 3px; }
"""

OPENINGS_FENS = [
    "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
    "rnbqkbnr/pppppppp/8/8/2C5/8/PP1PPPPP/RNBQKBNR b KQkq - 0 1",
    "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq - 0 1",
]

# Ignore UCI options that are automatically managed by the python-chess library
SKIP_OPTIONS = {"multipv", "ponder"}

# ---------- Data ----------
@dataclass
class EngineConfig:
    path: str = ""
    name: str = "Engine"
    workdir: str = ""
    options: Dict[str, object] = field(default_factory=dict)

@dataclass
class GameResult:
    game_no: int
    white: str
    black: str
    result: str
    termination: str
    pgn: str

@dataclass
class DetailedStats:
    total_games: int = 0
    a_wins: int = 0; a_draws: int = 0; a_losses: int = 0
    a_white_wins: int = 0; a_white_draws: int = 0; a_white_losses: int = 0
    a_black_wins: int = 0; a_black_draws: int = 0; a_black_losses: int = 0
    def score_a(self) -> float: return (self.a_wins + 0.5 * self.a_draws) / max(1, self.total_games)
    def elo_diff_a_minus_b(self) -> Optional[float]:
        s = self.score_a()
        if self.total_games == 0 or s <= 0 or s >= 1: return None
        return 400.0 * math.log10(s / (1 - s) + 1e-9)

# ---------- Board ----------
class ChessBoardWidget(QtWidgets.QWidget):
    move_made = QtCore.Signal(str)
    def __init__(self, parent=None):
        super().__init__(parent)
        self._fen = chess.STARTING_FEN
        self._last_move: Optional[Tuple[int, int]] = None
        self._flipped = False; self._interactive = False; self._selected: Optional[int] = None
        self.setMinimumSize(480, 480)
    def sizeHint(self) -> QtCore.QSize: return QtCore.QSize(560, 560)
    def set_fen(self, fen: str): self._fen = fen; self.update()
    def set_last_move(self, uci: Optional[str]):
        if uci: self._last_move = (chess.Move.from_uci(uci).from_square, chess.Move.from_uci(uci).to_square)
        else: self._last_move = None
        self.update()
    def set_interactive(self, on: bool): self._interactive = on; self._selected = None; self.update()
    def flip(self): self._flipped = not self._flipped; self.update()
    def _square_to_rc(self, sq: int) -> Tuple[int, int]: return 7 - (sq // 8), sq % 8
    def _rc_to_square(self, r: int, c: int) -> int: return (7 - r) * 8 + c
    def _view_coords(self, r: int, c: int) -> Tuple[int, int]: return (r, c) if not self._flipped else (7 - r, 7 - c)
    def _pos_to_square(self, x: int, y: int) -> Optional[int]:
        rect = self.rect(); size = min(rect.width(), rect.height()); sq_size = size // 8
        mx, my = (rect.width() - size) // 2, (rect.height() - size) // 2
        c, r = (x - mx) // sq_size, (y - my) // sq_size
        if not (0 <= c < 8 and 0 <= r < 8): return None
        r, c = (r, c) if not self._flipped else (7 - r, 7 - c)
        return self._rc_to_square(r, c)
    def mousePressEvent(self, e: QtGui.QMouseEvent) -> None:
        if not self._interactive: return
        sq = self._pos_to_square(int(e.position().x()), int(e.position().y()))
        if sq is None: return
        board = chess.Board(self._fen)
        if self._selected is None: self._selected = sq
        else:
            piece = board.piece_at(self._selected)
            is_promo = piece and piece.piece_type == chess.PAWN and chess.square_rank(sq) in [0, 7]
            mv = chess.Move(self._selected, sq, promotion=chess.QUEEN if is_promo else None)
            if mv in board.legal_moves: self.move_made.emit(mv.uci())
            else: self._selected = sq if board.piece_at(sq) and board.piece_at(sq).color == board.turn else None
        self.update()
    def paintEvent(self, ev: QtGui.QPaintEvent) -> None:
        p = QtGui.QPainter(self); p.setRenderHints(QtGui.QPainter.Antialiasing | QtGui.QPainter.TextAntialiasing)
        rect = self.rect(); size = min(rect.width(), rect.height()); board_rect = QtCore.QRect((rect.width()-size)//2, (rect.height()-size)//2, size, size)
        sq = size // 8; light, dark, hl, sel = QtGui.QColor("#2a2d2e"), QtGui.QColor("#1f2223"), QtGui.QColor("#3b82f6"), QtGui.QColor("#a855f7")
        for r in range(8):
            for c in range(8):
                vr, vc = self._view_coords(r, c)
                p.fillRect(board_rect.x() + vc*sq, board_rect.y() + vr*sq, sq, sq, light if (r+c)%2 == 0 else dark)
        if self._last_move:
            for s in self._last_move:
                r, c = self._square_to_rc(s); vr, vc = self._view_coords(r, c)
                p.fillRect(board_rect.x() + vc*sq, board_rect.y() + vr*sq, sq, sq, QtGui.QColor(hl.red(), hl.green(), hl.blue(), 90))
        if self._interactive and self._selected is not None:
            r, c = self._square_to_rc(self._selected); vr, vc = self._view_coords(r, c)
            p.fillRect(board_rect.x() + vc*sq, board_rect.y() + vr*sq, sq, sq, QtGui.QColor(sel.red(), sel.green(), sel.blue(), 80))
        board = chess.Board(self._fen)
        font = p.font(); font.setPointSize(int(sq*0.6)); p.setFont(font); p.setPen(QtGui.QColor("#e8e6e3"))
        pieces = {"K": "♔", "Q": "♕", "R": "♖", "B": "♗", "N": "♘", "P": "♙", "k": "♚", "q": "♛", "r": "♜", "b": "♝", "n": "♞", "p": "♟"}
        for s in chess.SQUARES:
            piece = board.piece_at(s)
            if piece:
                r,c = self._square_to_rc(s); vr, vc = self._view_coords(r, c)
                p.drawText(QtCore.QRectF(board_rect.x()+vc*sq, board_rect.y()+vr*sq, sq, sq), QtCore.Qt.AlignCenter, pieces[piece.symbol()])

# ---------- Engine wrapper ----------
class EngineWrapper(QtCore.QObject):
    def __init__(self, cfg: EngineConfig, parent=None):
        super().__init__(parent); self.cfg = cfg; self.proc: Optional[chess.engine.SimpleEngine] = None
    def start(self) -> None:
        if not os.path.isfile(self.cfg.path): raise FileNotFoundError(f"Engine not found: {self.cfg.path}")
        kwargs = {"cwd": self.cfg.workdir} if self.cfg.workdir and os.path.isdir(self.cfg.workdir) else {}
        self.proc = chess.engine.SimpleEngine.popen_uci(self.cfg.path, **kwargs)
        if self.cfg.options: self.configure(self.cfg.options)
    def configure(self, opts: Dict[str, object]) -> None:
        if self.proc:
            for k, v in opts.items():
                if k.lower() not in SKIP_OPTIONS and k in self.proc.options:
                    try:
                        self.proc.configure({k: v})
                    except (ValueError, TypeError, RuntimeError) as e:
                        print(f"Warning: Could not set option {k}={v}. Reason: {e}")

    def close(self):
        if self.proc:
            try: self.proc.quit()
            except chess.engine.EngineError: self.proc.kill()
        self.proc = None

# ---------- Match runner ----------
class MatchRunner(QtCore.QThread):
    sig_log = QtCore.Signal(str); sig_progress = QtCore.Signal(int, int); sig_result = QtCore.Signal(object)
    sig_summary = QtCore.Signal(object); sig_position = QtCore.Signal(str, str, str); sig_searchinfo = QtCore.Signal(dict)
    def __init__(self, cfg_a: EngineConfig, cfg_b: EngineConfig, games: int, movetime_ms: int, openings: list, parent=None):
        super().__init__(parent)
        self.cfg_a, self.cfg_b = cfg_a, cfg_b; self.games, self.movetime_s = games, max(0.01, movetime_ms/1000.0)
        self.openings = openings; self._stop = False
    def log(self, msg: str): self.sig_log.emit(msg)
    def stop(self): self._stop = True
    def get_move(self, eng: EngineWrapper, board: chess.Board, side_label: str) -> Optional[chess.Move]:
        assert eng.proc
        limit, best_move = chess.engine.Limit(time=self.movetime_s), None
        try:
            with eng.proc.analysis(board, limit) as analysis:
                for info in analysis:
                    if self._stop: return None
                    d = {"side": side_label}
                    if "depth" in info: d["depth"], d["seldepth"] = info["depth"], info.get("seldepth")
                    if "score" in info:
                        score = info["score"].white()
                        d["score_mate"], d["score_cp"] = (score.mate(), None) if score.is_mate() else (None, score.cp)
                    if "nps" in info: d["nps"] = info["nps"]
                    if "nodes" in info: d["nodes"] = info["nodes"]
                    if "hashfull" in info: d["hashfull"] = info["hashfull"]
                    if "time" in info: d["time_ms"] = int(info["time"]*1000)
                    if "pv" in info and info["pv"]: best_move = info["pv"][0]; d["pv"] = board.variation_san(info["pv"])
                    self.sig_searchinfo.emit(d)
        except chess.engine.EngineTerminatedError as e: self.log(f"Engine error: {e}"); return None
        return best_move if best_move else eng.proc.play(board, limit).move
    def run(self):
        stats = DetailedStats(); eA, eB = EngineWrapper(self.cfg_a), EngineWrapper(self.cfg_b)
        try: eA.start(); eB.start(); self.log(f"Engines ready: {eA.cfg.name} vs {eB.cfg.name}")
        except Exception as ex: self.log(f"Engine init error: {ex}"); eA.close(); eB.close(); return
        for game_no in range(1, self.games + 1):
            if self._stop: break
            board = chess.Board(random.choice(self.openings)); is_a_white = game_no % 2 == 1
            white_engine, black_engine = (eA, eB) if is_a_white else (eB, eA)
            white_name, black_name = (eA.cfg.name, eB.cfg.name) if is_a_white else (eB.cfg.name, eA.cfg.name)
            self.log(f"Game {game_no}/{self.games} | {white_name} (W) vs {black_name} (B)")
            game = chess.pgn.Game.from_board(board); game.headers.update({"White": white_name, "Black": black_name})
            self.sig_position.emit(board.fen(), "", str(game)); termination = "normal"
            try:
                while not board.is_game_over(claim_draw=True) and not self._stop and board.ply() < 500:
                    eng = white_engine if board.turn == chess.WHITE else black_engine
                    side = eA.cfg.name if eng == eA else eB.cfg.name
                    mv = self.get_move(eng, board, side)
                    if mv is None or self._stop: break
                    if mv not in board.legal_moves: termination = "illegal move"; break
                    board.push(mv); game.end().add_main_variation(mv)
                    self.sig_position.emit(board.fen(), mv.uci(), str(game))
            except Exception as ex: termination = f"engine error: {ex}"; self.log(f"Error: {termination}")
            if self._stop: termination = "stopped by user"
            result = board.result(claim_draw=True)
            if "error" in termination or "illegal" in termination: result = "0-1" if board.turn == chess.WHITE else "1-0"
            game.headers["Result"] = result; game.headers["Termination"] = termination; stats.total_games += 1
            if result == "1-0":
                if is_a_white: stats.a_wins += 1; stats.a_white_wins += 1
                else: stats.a_losses += 1; stats.a_black_losses += 1
            elif result == "0-1":
                if is_a_white: stats.a_losses += 1; stats.a_white_losses += 1
                else: stats.a_wins += 1; stats.a_black_wins += 1
            else:
                stats.a_draws += 1
                if is_a_white: stats.a_white_draws += 1
                else: stats.a_black_draws += 1
            self.sig_result.emit(GameResult(game_no, white_name, black_name, result, termination, str(game)))
            self.sig_progress.emit(stats.total_games, self.games); self.sig_summary.emit(stats)
        eA.close(); eB.close(); self.log("Match finished.")

# ---------- UI ----------
class EngineOptionsDialog(QtWidgets.QDialog):
    def __init__(self, cfg: EngineConfig, parent=None):
        super().__init__(parent); self.cfg = cfg; self.setWindowTitle(f"UCI Options for {cfg.name}"); self.setMinimumSize(600, 400)
        self.layout = QtWidgets.QVBoxLayout(self); self.table = QtWidgets.QTableWidget(); self.table.setColumnCount(2)
        self.table.setHorizontalHeaderLabels(["Option", "Value"])
        self.table.horizontalHeader().setSectionResizeMode(0, QtWidgets.QHeaderView.ResizeToContents)
        self.table.horizontalHeader().setSectionResizeMode(1, QtWidgets.QHeaderView.Stretch)
        self.table.verticalHeader().setSectionResizeMode(QtWidgets.QHeaderView.ResizeToContents)
        btn_detect = QtWidgets.QPushButton("Detect Options from Engine"); btn_detect.clicked.connect(self.detect_options)
        self.buttons = QtWidgets.QDialogButtonBox(QtWidgets.QDialogButtonBox.Ok | QtWidgets.QDialogButtonBox.Cancel)
        self.buttons.accepted.connect(self.save_options); self.buttons.rejected.connect(self.reject)
        self.layout.addWidget(btn_detect); self.layout.addWidget(self.table); self.layout.addWidget(self.buttons)
        self.populate_table()
    def detect_options(self):
        if not os.path.isfile(self.cfg.path): QtWidgets.QMessageBox.warning(self, "Error", f"Engine path not found: {self.cfg.path}"); return
        try:
            engine = chess.engine.SimpleEngine.popen_uci(self.cfg.path)
            new_opts = {name: self.cfg.options.get(name, opt.default) for name, opt in engine.options.items()}
            engine.quit(); self.cfg.options = new_opts; self.populate_table()
        except Exception as e: QtWidgets.QMessageBox.critical(self, "Detection Failed", f"Could not get options from engine:\n{e}")
    def populate_table(self):
        self.table.setRowCount(0); opts = sorted(self.cfg.options.items()); self.table.setRowCount(len(opts))
        for row, (name, val) in enumerate(opts):
            self.table.setItem(row, 0, QtWidgets.QTableWidgetItem(name))
            if isinstance(val, bool): w = QtWidgets.QCheckBox(); w.setChecked(val)
            elif isinstance(val, int): w = QtWidgets.QSpinBox(); w.setRange(-1000000, 1000000); w.setValue(val)
            else: w = QtWidgets.QLineEdit(str(val))
            self.table.setCellWidget(row, 1, w)
    def save_options(self):
        for row in range(self.table.rowCount()):
            name, widget = self.table.item(row, 0).text(), self.table.cellWidget(row, 1)
            if isinstance(widget, QtWidgets.QCheckBox): self.cfg.options[name] = widget.isChecked()
            elif isinstance(widget, QtWidgets.QSpinBox): self.cfg.options[name] = widget.value()
            elif isinstance(widget, QtWidgets.QLineEdit): self.cfg.options[name] = widget.text()
        self.accept()

class MainWindow(QtWidgets.QMainWindow):
    def __init__(self):
        super().__init__(); self.setWindowTitle(f"{APP_TITLE} — v{VERSION}"); self.resize(1400, 900)
        self.presets = self.load_presets()
        self.cfg_a = EngineConfig(name="Engine A"); self.cfg_b = EngineConfig(name="Engine B")
        self.runner: Optional[MatchRunner] = None; self.results: List[GameResult] = []
        
        self.white_info_group: Optional[QtWidgets.QGroupBox] = None
        self.black_info_group: Optional[QtWidgets.QGroupBox] = None
        self.white_info_labels: Dict[str, QtWidgets.QLabel] = {}
        self.black_info_labels: Dict[str, QtWidgets.QLabel] = {}
        self.current_white_name: Optional[str] = None
        self.current_black_name: Optional[str] = None

        self.current_pgn_game: Optional[chess.pgn.Game] = None
        self.current_game_node: Optional[chess.pgn.GameNode] = None

        self.tabs = QtWidgets.QTabWidget(); self.setCentralWidget(self.tabs)
        self.tabs.addTab(self.build_engine_setup_tab(), "Engine Setup")
        self.tabs.addTab(self.build_live_match_tab(), "Live Match")
        self.tabs.addTab(self.build_stats_tab(), "Match Statistics")
        self.apply_dark_palette()
        self.refresh_presets_combos()
    def apply_dark_palette(self): self.setStyleSheet(DARK_QSS)
    def build_engine_setup_tab(self) -> QtWidgets.QWidget:
        w = QtWidgets.QWidget(); root_layout = QtWidgets.QVBoxLayout(w); grid = QtWidgets.QGridLayout()
        # Engine A
        group_a = QtWidgets.QGroupBox("Engine A Configuration"); layout_a = QtWidgets.QGridLayout(group_a)
        self.cmb_preset_a = QtWidgets.QComboBox(); self.btn_load_a = QtWidgets.QPushButton("Load")
        self.ed_name_a = QtWidgets.QLineEdit(self.cfg_a.name); self.ed_path_a = QtWidgets.QLineEdit(); self.btn_browse_path_a = QtWidgets.QPushButton("…")
        self.ed_wd_a = QtWidgets.QLineEdit(); self.btn_browse_wd_a = QtWidgets.QPushButton("…")
        self.btn_opts_a = QtWidgets.QPushButton("Configure UCI Options…"); self.btn_save_a = QtWidgets.QPushButton("Save as Preset…")
        path_l_a = QtWidgets.QHBoxLayout(); path_l_a.addWidget(self.ed_path_a); path_l_a.addWidget(self.btn_browse_path_a)
        wd_l_a = QtWidgets.QHBoxLayout(); wd_l_a.addWidget(self.ed_wd_a); wd_l_a.addWidget(self.btn_browse_wd_a)
        layout_a.addWidget(QtWidgets.QLabel("Preset:"), 0, 0); layout_a.addWidget(self.cmb_preset_a, 0, 1); layout_a.addWidget(self.btn_load_a, 0, 2)
        layout_a.addWidget(QtWidgets.QLabel("Name:"), 1, 0); layout_a.addWidget(self.ed_name_a, 1, 1, 1, 2)
        layout_a.addWidget(QtWidgets.QLabel("Path:"), 2, 0); layout_a.addLayout(path_l_a, 2, 1, 1, 2)
        layout_a.addWidget(QtWidgets.QLabel("Work Dir:"), 3, 0); layout_a.addLayout(wd_l_a, 3, 1, 1, 2)
        layout_a.addWidget(self.btn_opts_a, 4, 0, 1, 3); layout_a.addWidget(self.btn_save_a, 5, 0, 1, 3)
        # Engine B
        group_b = QtWidgets.QGroupBox("Engine B Configuration"); layout_b = QtWidgets.QGridLayout(group_b)
        self.cmb_preset_b = QtWidgets.QComboBox(); self.btn_load_b = QtWidgets.QPushButton("Load")
        self.ed_name_b = QtWidgets.QLineEdit(self.cfg_b.name); self.ed_path_b = QtWidgets.QLineEdit(); self.btn_browse_path_b = QtWidgets.QPushButton("…")
        self.ed_wd_b = QtWidgets.QLineEdit(); self.btn_browse_wd_b = QtWidgets.QPushButton("…")
        self.btn_opts_b = QtWidgets.QPushButton("Configure UCI Options…"); self.btn_save_b = QtWidgets.QPushButton("Save as Preset…")
        path_l_b = QtWidgets.QHBoxLayout(); path_l_b.addWidget(self.ed_path_b); path_l_b.addWidget(self.btn_browse_path_b)
        wd_l_b = QtWidgets.QHBoxLayout(); wd_l_b.addWidget(self.ed_wd_b); wd_l_b.addWidget(self.btn_browse_wd_b)
        layout_b.addWidget(QtWidgets.QLabel("Preset:"), 0, 0); layout_b.addWidget(self.cmb_preset_b, 0, 1); layout_b.addWidget(self.btn_load_b, 0, 2)
        layout_b.addWidget(QtWidgets.QLabel("Name:"), 1, 0); layout_b.addWidget(self.ed_name_b, 1, 1, 1, 2)
        layout_b.addWidget(QtWidgets.QLabel("Path:"), 2, 0); layout_b.addLayout(path_l_b, 2, 1, 1, 2)
        layout_b.addWidget(QtWidgets.QLabel("Work Dir:"), 3, 0); layout_b.addLayout(wd_l_b, 3, 1, 1, 2)
        layout_b.addWidget(self.btn_opts_b, 4, 0, 1, 3); layout_b.addWidget(self.btn_save_b, 5, 0, 1, 3)
        
        self.btn_del_preset = QtWidgets.QPushButton("Delete Selected Preset")
        grid.addWidget(group_a, 0, 0); grid.addWidget(group_b, 0, 1); root_layout.addLayout(grid); root_layout.addWidget(self.btn_del_preset)
        self.btn_load_a.clicked.connect(lambda: self.load_preset_to('A')); self.btn_browse_path_a.clicked.connect(lambda: self.pick_file(self.ed_path_a)); self.btn_browse_wd_a.clicked.connect(lambda: self.pick_dir(self.ed_wd_a)); self.btn_opts_a.clicked.connect(lambda: self.configure_engine_options(self.cfg_a, self.ed_name_a, self.ed_path_a)); self.btn_save_a.clicked.connect(lambda: self.save_current_as_preset('A'))
        self.btn_load_b.clicked.connect(lambda: self.load_preset_to('B')); self.btn_browse_path_b.clicked.connect(lambda: self.pick_file(self.ed_path_b)); self.btn_browse_wd_b.clicked.connect(lambda: self.pick_dir(self.ed_wd_b)); self.btn_opts_b.clicked.connect(lambda: self.configure_engine_options(self.cfg_b, self.ed_name_b, self.ed_path_b)); self.btn_save_b.clicked.connect(lambda: self.save_current_as_preset('B'))
        self.btn_del_preset.clicked.connect(self.delete_selected_preset)
        return w
    def build_live_match_tab(self) -> QtWidgets.QWidget:
        w = QtWidgets.QWidget(); root = QtWidgets.QHBoxLayout(w); left_vbox = QtWidgets.QVBoxLayout()
        controls_group = QtWidgets.QGroupBox("Match Controls"); cl = QtWidgets.QFormLayout(controls_group)
        self.spn_games = QtWidgets.QSpinBox(); self.spn_games.setRange(1, 10000); self.spn_games.setValue(20)
        self.spn_movetime = QtWidgets.QSpinBox(); self.spn_movetime.setRange(10, 60000); self.spn_movetime.setValue(1000)
        self.cmb_openings_mode = QtWidgets.QComboBox(); self.cmb_openings_mode.addItems(["Start Position", "3 Random FENs"])
        self.btn_start = QtWidgets.QPushButton("Start Match"); self.btn_stop = QtWidgets.QPushButton("Stop Match"); self.btn_stop.setEnabled(False)
        btn_layout = QtWidgets.QHBoxLayout(); btn_layout.addWidget(self.btn_start); btn_layout.addWidget(self.btn_stop)
        cl.addRow("Games:", self.spn_games); cl.addRow("Time/Move (ms):", self.spn_movetime); cl.addRow("Openings:", self.cmb_openings_mode); cl.addRow(btn_layout)
        self.progress = QtWidgets.QProgressBar(); self.progress.setValue(0); self.txt_log = QtWidgets.QPlainTextEdit(); self.txt_log.setReadOnly(True)
        left_vbox.addWidget(controls_group); left_vbox.addWidget(self.progress); left_vbox.addWidget(self.txt_log, 1)
        
        center_vbox = QtWidgets.QVBoxLayout(); self.board = ChessBoardWidget()
        nav_layout = QtWidgets.QHBoxLayout()
        self.btn_nav_first = QtWidgets.QPushButton("|<"); self.btn_nav_prev = QtWidgets.QPushButton("<")
        self.btn_nav_next = QtWidgets.QPushButton(">"); self.btn_nav_last = QtWidgets.QPushButton(">|")
        nav_layout.addWidget(self.btn_nav_first); nav_layout.addWidget(self.btn_nav_prev)
        nav_layout.addWidget(self.btn_nav_next); nav_layout.addWidget(self.btn_nav_last)
        self.lbl_live = QtWidgets.QLabel("Live: —"); self.lbl_live.setAlignment(QtCore.Qt.AlignCenter)
        center_vbox.addWidget(self.board, 1); center_vbox.addLayout(nav_layout); center_vbox.addWidget(self.lbl_live)
        
        right_vbox = QtWidgets.QVBoxLayout()
        info_panel = self.build_info_panels()
        self.txt_pgn = QtWidgets.QPlainTextEdit(); self.txt_pgn.setReadOnly(True)
        right_vbox.addWidget(info_panel)
        right_vbox.addWidget(self.txt_pgn, 1)

        splitter = QtWidgets.QSplitter(QtCore.Qt.Horizontal); left_w, center_w, right_w = QtWidgets.QWidget(), QtWidgets.QWidget(), QtWidgets.QWidget()
        left_w.setLayout(left_vbox); center_w.setLayout(center_vbox); right_w.setLayout(right_vbox)
        splitter.addWidget(left_w); splitter.addWidget(center_w); splitter.addWidget(right_w); splitter.setSizes([300, 600, 300]); root.addWidget(splitter)
        self.btn_start.clicked.connect(self.on_start); self.btn_stop.clicked.connect(self.on_stop)
        self.btn_nav_first.clicked.connect(self.nav_first); self.btn_nav_prev.clicked.connect(self.nav_prev)
        self.btn_nav_next.clicked.connect(self.nav_next); self.btn_nav_last.clicked.connect(self.nav_last)
        self.update_nav_buttons_state()
        return w
    def _create_info_group(self, title: str) -> Tuple[QtWidgets.QGroupBox, Dict[str, QtWidgets.QLabel]]:
        group = QtWidgets.QGroupBox(title)
        grid = QtWidgets.QFormLayout(group)
        labels = {
            "side": QtWidgets.QLabel("—"), "depth": QtWidgets.QLabel("—"), "score": QtWidgets.QLabel("—"),
            "nps": QtWidgets.QLabel("—"), "nodes": QtWidgets.QLabel("—"), "hash": QtWidgets.QLabel("—"),
            "time": QtWidgets.QLabel("—"), "pv": QtWidgets.QLabel("—")
        }
        labels["pv"].setWordWrap(True)
        grid.addRow("Side:", labels["side"]); grid.addRow("Depth/SelDepth:", labels["depth"]); grid.addRow("Score:", labels["score"]); grid.addRow("NPS:", labels["nps"])
        grid.addRow("Nodes:", labels["nodes"]); grid.addRow("Hash:", labels["hash"]); grid.addRow("Time:", labels["time"]); grid.addRow("PV:", labels["pv"])
        return group, labels
    def build_info_panels(self) -> QtWidgets.QWidget:
        container = QtWidgets.QWidget()
        layout = QtWidgets.QVBoxLayout(container)
        layout.setContentsMargins(0,0,0,0)
        
        self.white_info_group, self.white_info_labels = self._create_info_group("White Engine")
        self.black_info_group, self.black_info_labels = self._create_info_group("Black Engine")

        layout.addWidget(self.white_info_group)
        layout.addWidget(self.black_info_group)
        layout.addStretch()
        return container
    def build_stats_tab(self) -> QtWidgets.QWidget:
        w = QtWidgets.QWidget(); root = QtWidgets.QVBoxLayout(w); summary_group = QtWidgets.QGroupBox("Summary"); sgrid = QtWidgets.QGridLayout(summary_group)
        self.stat_total_games, self.stat_elo = QtWidgets.QLabel("0"), QtWidgets.QLabel("N/A")
        self.stat_a_score, self.stat_a_white, self.stat_a_black = QtWidgets.QLabel("0 / 0 / 0"), QtWidgets.QLabel("0 / 0 / 0"), QtWidgets.QLabel("0 / 0 / 0")
        
        self.stat_a_score_label = QtWidgets.QLabel(f"<b>Score (Engine A):</b>")
        self.stat_a_white_label = QtWidgets.QLabel(f"<b>Engine A as White:</b>")
        self.stat_a_black_label = QtWidgets.QLabel(f"<b>Engine A as Black:</b>")

        sgrid.addWidget(QtWidgets.QLabel("<b>Total Games:</b>"), 0, 0); sgrid.addWidget(self.stat_total_games, 0, 1); sgrid.addWidget(QtWidgets.QLabel("<b>Elo Diff (A-B):</b>"), 0, 2); sgrid.addWidget(self.stat_elo, 0, 3)
        sgrid.addWidget(self.stat_a_score_label, 1, 0); sgrid.addWidget(self.stat_a_score, 1, 1)
        sgrid.addWidget(self.stat_a_white_label, 2, 0); sgrid.addWidget(self.stat_a_white, 2, 1)
        sgrid.addWidget(self.stat_a_black_label, 3, 0); sgrid.addWidget(self.stat_a_black, 3, 1)
        
        sgrid.setColumnStretch(1, 1); sgrid.setColumnStretch(3, 1)
        self.results_table = QtWidgets.QTableWidget(); self.results_table.setColumnCount(5); self.results_table.setHorizontalHeaderLabels(["#", "White", "Black", "Result", "Termination"])
        self.results_table.horizontalHeader().setSectionResizeMode(QtWidgets.QHeaderView.Stretch); self.results_table.setEditTriggers(QtWidgets.QAbstractItemView.NoEditTriggers)
        root.addWidget(summary_group); root.addWidget(self.results_table)
        return w
    def pick_file(self, line: QtWidgets.QLineEdit): path, _ = QtWidgets.QFileDialog.getOpenFileName(self, "Select File"); line.setText(path) if path else None
    def pick_dir(self, line: QtWidgets.QLineEdit): path = QtWidgets.QFileDialog.getExistingDirectory(self, "Select Directory"); line.setText(path) if path else None
    def configure_engine_options(self, cfg: EngineConfig, name_edit: QtWidgets.QLineEdit, path_edit: QtWidgets.QLineEdit):
        cfg.name, cfg.path = name_edit.text(), path_edit.text(); EngineOptionsDialog(cfg, self).exec()
    def on_start(self):
        self.gather_configs()
        if not all(os.path.isfile(c.path) for c in [self.cfg_a, self.cfg_b]): QtWidgets.QMessageBox.warning(self, "Missing engines", "Select valid engine paths on the Setup tab."); return
        
        self.stat_a_score_label.setText(f"<b>Score ({self.cfg_a.name}):</b>"); self.stat_a_white_label.setText(f"<b>{self.cfg_a.name} as White:</b>"); self.stat_a_black_label.setText(f"<b>{self.cfg_a.name} as Black:</b>")
        self.current_pgn_game = None; self.current_game_node = None
        self.update_nav_buttons_state()

        self.btn_start.setEnabled(False); self.btn_stop.setEnabled(True); self.tabs.setCurrentIndex(1)
        self.results.clear(); self.txt_log.clear(); self.results_table.setRowCount(0); self.progress.setValue(0)
        openings = [chess.STARTING_FEN] if self.cmb_openings_mode.currentText() == "Start Position" else random.sample(OPENINGS_FENS, k=3)
        self.runner = MatchRunner(self.cfg_a, self.cfg_b, self.spn_games.value(), self.spn_movetime.value(), openings, self)
        self.runner.sig_log.connect(self.append_log); self.runner.sig_progress.connect(self.update_progress); self.runner.sig_result.connect(self.on_game_result)
        self.runner.sig_summary.connect(self.update_stats); self.runner.sig_position.connect(self.on_position); self.runner.sig_searchinfo.connect(self.on_searchinfo)
        self.runner.finished.connect(lambda: (self.btn_start.setEnabled(True), self.btn_stop.setEnabled(False))); self.runner.start()
    def on_stop(self):
        if self.runner: self.runner.stop()
        self.btn_start.setEnabled(True); self.btn_stop.setEnabled(False)
    def gather_configs(self):
        self.cfg_a.name, self.cfg_a.path, self.cfg_a.workdir = self.ed_name_a.text(), self.ed_path_a.text(), self.ed_wd_a.text()
        self.cfg_b.name, self.cfg_b.path, self.cfg_b.workdir = self.ed_name_b.text(), self.ed_path_b.text(), self.ed_wd_b.text()
    @QtCore.Slot(str)
    def append_log(self, msg: str): self.txt_log.appendPlainText(msg)
    @QtCore.Slot(int, int)
    def update_progress(self, d: int, t: int): self.progress.setValue(int(100*d/max(1,t)))
    @QtCore.Slot(object)
    def on_game_result(self, gr: GameResult):
        self.results.append(gr); row = self.results_table.rowCount(); self.results_table.insertRow(row)
        for i, item in enumerate([gr.game_no, gr.white, gr.black, gr.result, gr.termination]): self.results_table.setItem(row, i, QtWidgets.QTableWidgetItem(str(item)))
    @QtCore.Slot(object)
    def update_stats(self, s: DetailedStats):
        self.stat_total_games.setText(f"{s.total_games}")
        self.stat_elo.setText(f"{s.elo_diff_a_minus_b():+.1f}" if s.elo_diff_a_minus_b() is not None else "N/A")
        self.stat_a_score.setText(f"{s.a_wins} / {s.a_draws} / {s.a_losses} ({s.score_a():.1%})")
        self.stat_a_white.setText(f"{s.a_white_wins} / {s.a_white_draws} / {s.a_white_losses}")
        self.stat_a_black.setText(f"{s.a_black_wins} / {s.a_black_draws} / {s.a_black_losses}")
    @QtCore.Slot(str, str, str)
    def on_position(self, fen: str, uci: str, pgn: str):
        self.board.set_fen(fen); self.board.set_last_move(uci or None); self.txt_pgn.setPlainText(pgn)
        
        is_new_game = not uci
        if is_new_game:
            self.current_pgn_game = chess.pgn.read_game(io.StringIO(pgn))
            self.current_game_node = self.current_pgn_game
            
            for labels in [self.white_info_labels, self.black_info_labels]:
                for label in labels.values(): label.setText("—")
            
            self.current_white_name = self.current_pgn_game.headers.get("White", "White")
            self.current_black_name = self.current_pgn_game.headers.get("Black", "Black")
            if self.white_info_group: self.white_info_group.setTitle(f"White: {self.current_white_name}")
            if self.black_info_group: self.black_info_group.setTitle(f"Black: {self.current_black_name}")
        elif self.current_game_node and not self.current_game_node.is_end():
            self.current_game_node = self.current_game_node.variation(0)

        b = self.current_game_node.board() if self.current_game_node else chess.Board(fen)
        self.lbl_live.setText(f"{b.fullmove_number}. {'White' if b.turn == chess.WHITE else 'Black'} to move")
        self.update_nav_buttons_state()
    def update_board_from_node(self):
        if not self.current_game_node: return
        board = self.current_game_node.board()
        self.board.set_fen(board.fen())
        move = self.current_game_node.move
        self.board.set_last_move(move.uci() if move else None)
        self.lbl_live.setText(f"{board.fullmove_number}. {'White' if board.turn == chess.WHITE else 'Black'} to move")
        self.update_nav_buttons_state()
    def update_nav_buttons_state(self):
        has_game = self.current_game_node is not None
        self.btn_nav_first.setEnabled(has_game and self.current_game_node.parent is not None)
        self.btn_nav_prev.setEnabled(has_game and self.current_game_node.parent is not None)
        self.btn_nav_next.setEnabled(has_game and not self.current_game_node.is_end())
        self.btn_nav_last.setEnabled(has_game and not self.current_game_node.is_end())
    def nav_first(self):
        if self.current_pgn_game: self.current_game_node = self.current_pgn_game; self.update_board_from_node()
    def nav_prev(self):
        if self.current_game_node and self.current_game_node.parent: self.current_game_node = self.current_game_node.parent; self.update_board_from_node()
    def nav_next(self):
        if self.current_game_node and not self.current_game_node.is_end(): self.current_game_node = self.current_game_node.variation(0); self.update_board_from_node()
    def nav_last(self):
        if self.current_game_node:
            node = self.current_game_node
            while not node.is_end(): node = node.variation(0)
            self.current_game_node = node; self.update_board_from_node()
    def _update_info_panel(self, labels: Dict[str, QtWidgets.QLabel], d: dict):
        labels["side"].setText(d.get("side", "—"))
        depth, seldepth = d.get("depth"), d.get("seldepth")
        labels["depth"].setText(f"{depth}/{seldepth}" if depth is not None else "—")
        
        if d.get("score_mate") is not None: labels["score"].setText(f"Mate {d['score_mate']}")
        elif d.get("score_cp") is not None: labels["score"].setText(f"{d['score_cp']/100:.2f}")
        
        for key in ["nps", "nodes", "pv"]: labels[key].setText(str(d.get(key, "—")))
        labels["hash"].setText(f"{d['hashfull']/10:.1f}%" if "hashfull" in d else "—")
        labels["time"].setText(f"{d['time_ms']} ms" if "time_ms" in d else "—")
    @QtCore.Slot(dict)
    def on_searchinfo(self, d: dict):
        side = d.get("side")
        if side == self.current_white_name:
            self._update_info_panel(self.white_info_labels, d)
        elif side == self.current_black_name:
            self._update_info_panel(self.black_info_labels, d)
    def load_presets(self) -> Dict[str, EngineConfig]:
        if not os.path.isfile(PRESETS_PATH): return {}
        try:
            with open(PRESETS_PATH, "r", encoding="utf-8") as f: data = json.load(f)
            return {k: EngineConfig(**v) for k, v in data.items()}
        except Exception: return {}
    def save_presets_file(self):
        data = {k: v.__dict__ for k, v in self.presets.items()}
        with open(PRESETS_PATH, "w", encoding="utf-8") as f: json.dump(data, f, indent=2)
    def refresh_presets_combos(self):
        keys = ["(Select Preset)"] + sorted(self.presets.keys())
        self.cmb_preset_a.clear(); self.cmb_preset_a.addItems(keys)
        self.cmb_preset_b.clear(); self.cmb_preset_b.addItems(keys)
    def load_preset_to(self, which: str):
        combo, name_ed, path_ed, wd_ed, cfg = (self.cmb_preset_a, self.ed_name_a, self.ed_path_a, self.ed_wd_a, self.cfg_a) if which == 'A' else (self.cmb_preset_b, self.ed_name_b, self.ed_path_b, self.ed_wd_b, self.cfg_b)
        key = combo.currentText()
        if key not in self.presets: return
        preset_cfg = self.presets[key]; name_ed.setText(preset_cfg.name); path_ed.setText(preset_cfg.path); wd_ed.setText(preset_cfg.workdir); cfg.options = preset_cfg.options.copy()
        QtWidgets.QMessageBox.information(self, "Preset Loaded", f"Loaded '{key}' into Engine {which}.")
    def save_current_as_preset(self, which: str):
        name_ed, path_ed, wd_ed, cfg = (self.ed_name_a, self.ed_path_a, self.ed_wd_a, self.cfg_a) if which == 'A' else (self.ed_name_b, self.ed_path_b, self.ed_wd_b, self.cfg_b)
        if not path_ed.text(): QtWidgets.QMessageBox.warning(self, "Missing Path", "Set engine path before saving."); return
        key, ok = QtWidgets.QInputDialog.getText(self, "Preset Name", "Enter a name for this preset:", text=name_ed.text())
        if not (ok and key): return
        self.presets[key] = EngineConfig(path=path_ed.text(), name=name_ed.text(), workdir=wd_ed.text(), options=cfg.options)
        self.save_presets_file(); self.refresh_presets_combos()
        QtWidgets.QMessageBox.information(self, "Saved", f"Preset '{key}' saved.")
    def delete_selected_preset(self):
        key = self.cmb_preset_a.currentText()
        if key not in self.presets: key = self.cmb_preset_b.currentText()
        if key not in self.presets: QtWidgets.QMessageBox.warning(self, "No Preset Selected", "Select a preset to delete from a dropdown first."); return
        if QtWidgets.QMessageBox.question(self, "Confirm Delete", f"Are you sure you want to delete preset '{key}'?") == QtWidgets.QMessageBox.Yes:
            del self.presets[key]; self.save_presets_file(); self.refresh_presets_combos()
            QtWidgets.QMessageBox.information(self, "Deleted", f"Preset '{key}' removed.")
            
    def closeEvent(self, e: QtGui.QCloseEvent) -> None:
        if self.runner and self.runner.isRunning():
            self.runner.stop()
            self.runner.wait(2000) # Wait up to 2 seconds for the thread to finish
        super().closeEvent(e)

if __name__ == "__main__":
    app = QtWidgets.QApplication(sys.argv)
    app.setApplicationName(APP_TITLE)
    win = MainWindow()
    win.show()
    sys.exit(app.exec())
