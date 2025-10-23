# Rust Chess Engine

A small, clean chess engine with three entry points:

- `perft` — count nodes for debugging
- `play-cli` — play in the terminal (UCI move input like `e2e4`)
- `uci` — speak UCI for GUIs or engine-vs-engine

## Build

```bash
cargo build --release
```

## Run
##### Perft from start position
```bash
cargo run --release -- perft 3
```

##### Perft with a FEN
```bash
cargo run --release -- perft 4 --fen "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq c6 0 2"
```

##### Divide
```bash
cargo run --release -- perft 3 --divide
```

##### Play in terminal
```bash
cargo run --release -- play-cli --depth 3
```

##### UCI Mode
```bash
cargo run --release -- uci
```
