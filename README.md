# Chess Engine in Rust

This is a computer program that plays chess, written in the Rust programming language. It is designed to be simple, correct, and reasonably strong.

You can interact with this engine in several ways:
*   Play a game against it directly in your terminal.
*   Have it play games against itself.
*   Connect it to a graphical chess program (like Arena, Scid vs. PC, or Cute Chess) to play with a visual board.

## Features

*   **Standard Chess Rules:** Implements all rules, including castling, pawn promotions, and en passant.
*   **Smart Move Search:** Uses a common technique (alpha-beta search) to look ahead and find good moves.
*   **Opening Book:** Knows how to play the first few moves of many common openings for a strong start.
*   **Move Generation Testing:** Includes a `perft` tool to ensure the engine's move calculations are accurate.
*   **UCI Protocol:** Can communicate with chess graphical user interfaces (GUIs) using the standard Universal Chess Interface (UCI).

## How to Use

First, you will need to have the Rust programming language toolchain installed. Then, you can build the engine with a single command:

```bash
cargo build --release
```

The compiled program will be located in the `target/release/` directory.

### Playing a Game in the Terminal

To play against the engine, use the `play-cli` command. You can tell the engine how much time it should think for each move.

```bash
# Start a game where the engine thinks for up to 5 seconds per move
cargo run --release -- play-cli --time 5000
```

You can enter your moves in algebraic notation (e.g., `e2e4`, `g1f3`).

### Watching the Engine Play Itself

You can make the engine play a series of games against itself. This is useful for testing.

```bash
# The engine will play 5 games against itself, thinking 1 second per move
cargo run --release -- self-play --rounds 5 --time 1000
```

### Using with a Chess GUI

To use this engine with a graphical interface, run it in `uci` mode:

```bash
cargo run --release -- uci
```

Then, open your favorite chess GUI, go to the engine settings, and add a new UCI engine. When prompted, select the executable file for this program.

### Testing Move Generation (`perft`)

The `perft` command is a debugging tool that counts all possible moves up to a certain depth. It's a way to verify that the engine's understanding of the rules of chess is correct.

```bash
# Calculate all possible legal moves up to a depth of 5 from the starting position
cargo run --release -- perft 5
```

You can also test from any board position by providing a FEN string:

```bash
cargo run --release -- perft 4 --fen "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
```

## Acknowledgements

The opening book data is sourced from "The Baron's Polyglot Opening Book," which can be found on the [Chess Programming Wiki](https://www.chessprogramming.net/new-version-of-the-baron-v3-43-plus-the-barons-polyglot-opening-book/).
