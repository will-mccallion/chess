# Chess Engine in Rust

This is a chess engine written in Rust. It’s designed to be simple, correct, and reasonably strong, while still being easy to experiment with and extend.

You can interact with the engine in several ways:

- Play a game against it directly in your terminal.
- Let it play games against itself.
- Connect it to a graphical chess program (Arena, Scid vs. PC, Cute Chess, etc.) and use a visual board via UCI.

---

## Features

- **Standard Chess Rules**  
  Implements all standard rules, including castling, pawn promotion, en passant, threefold repetition, and the fifty-move rule.

- **Smart Move Search**  
  Uses alpha–beta search (with typical pruning and move-ordering techniques) to look ahead and find strong moves.

- **Opening Book**  
  Uses a Polyglot opening book to play many common openings confidently from move one.

- **Move Generation Testing (`perft`)**  
  Includes a `perft` tool to verify the correctness of move generation by counting all legal move sequences to a given depth.

- **UCI Protocol Support**  
  Speaks the Universal Chess Interface (UCI) protocol, so it works with most modern chess GUIs.

- **Optional NNUE Evaluation**  
  Supports loading an NNUE network file for more sophisticated evaluation (see **NNUE Support** below).

---

## Building

You’ll need the Rust toolchain installed (e.g. via [`rustup`](https://rustup.rs/)). It requires the nightly build for the SIMD functions.

From the project root, build in release mode:

```bash
cargo build --release
````

The compiled executable will be located in the `target/release/` directory.

---

## How to Use

All examples below assume you’re running commands from the project root.

### Playing a Game in the Terminal

To play against the engine, use the `play-cli` command and specify how much time (in milliseconds) the engine may think for each move:

```bash
# Start a game where the engine thinks for up to 5 seconds per move
cargo run --release -- play-cli --time 5000
```

Enter moves in UCI-style coordinate notation, for example: `e2e4`, `g1f3`, etc.

---

### Watching the Engine Play Itself

You can have the engine play a series of games against itself, which is useful for testing changes or just watching it in action:

```bash
# The engine will play 5 games against itself, thinking 1 second per move
cargo run --release -- self-play --rounds 5 --time 1000
```

---

### Using the Engine with a Chess GUI (UCI)

To use this engine with a graphical interface, start it in `uci` mode:

```bash
cargo run --release -- uci
```

Then, in your favorite chess GUI:

1. Open the engine management/settings dialog.
2. Add a **new UCI engine**.
3. When prompted for the engine executable, point to the binary (for example, `target/release/chess`).
4. Save the configuration and start a game using this engine.

---

### Testing Move Generation (`perft`)

The `perft` command is a debugging tool that counts all possible legal move sequences up to a given depth. This is an effective way to verify that the engine’s understanding of the rules is correct.

From the standard starting position:

```bash
# Calculate all possible legal moves up to a depth of 5 from the starting position
cargo run --release -- perft 5
```

From a custom FEN position:

```bash
cargo run --release -- perft 4 --fen "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
```

---

## NNUE Support

This engine can optionally use an NNUE (Efficiently Updatable Neural Network) file for evaluation.

* The default network file can be downloaded from:
  `https://tests.stockfishchess.org/api/nn/nn-9931db908a9b.nnue`

* Place the downloaded `.nnue` file where your configuration/evaluation code expects it (for example, alongside the engine binary or in the configured directory), and ensure the file name in the code matches the downloaded file name.

The NNUE parsing and evaluation code is largely adapted from:
[https://github.com/github-jimjim/NNUE-Parser.git](https://github.com/github-jimjim/NNUE-Parser.git)

---

## Acknowledgements

* **Opening Book**
  The opening book data is sourced from *The Baron's Polyglot Opening Book*, available via the
  [Chess Programming Wiki](https://www.chessprogramming.net/new-version-of-the-baron-v3-43-plus-the-barons-polyglot-opening-book/).

* **NNUE Parser**
  NNUE-related code is heavily inspired by and based on
  [https://github.com/github-jimjim/NNUE-Parser.git](https://github.com/github-jimjim/NNUE-Parser.git).
