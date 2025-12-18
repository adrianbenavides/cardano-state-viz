# Cardano State Machine Visualizer

A terminal-based tool for visualizing and analyzing Cardano smart contract state machines. Track UTXO states,
transitions, and datum evolution through an interactive TUI or export to various formats.

[![asciicast](https://asciinema.org/a/FJnlMlMEI3EWL5jojqIs5Sy1D.svg)](https://asciinema.org/a/FJnlMlMEI3EWL5jojqIs5Sy1D)

## Features

- 📊 **State Graph Building** - Automatically construct state transition graphs from on-chain transactions
- 🖥️ **Interactive TUI** - Navigate states, inspect datums, and view transactions in a rich terminal interface
- 📈 **Multiple Output Formats** - JSON, tables, Graphviz DOT, or interactive TUI
- 🔍 **Datum Inspector** - View raw CBOR hex or decoded PlutusData structures
- 🧠 **Pattern Analysis** - Detect structural patterns like linear timelines, trees, or cycles
- 📝 **Schema Support** - Define custom schemas for human-readable field names and classifications
- 🎨 **Color-Coded States** - Visual distinction between states types

## Installation

### Prerequisites

- Rust: install from [rustup.rs](https://rustup.rs)
- Graphviz: optional, for DOT format visualization

### Build from Source

```bash
git clone <repository-url>
cd cardano-state-viz
cargo build --release
```

The binary will be available at `target/release/cardano-state-viz`.

## Quick Start

### Analyze a Contract with Mock Data

```bash
# View in interactive TUI
cargo run -- analyze --address mock --output tui

# Export to DOT format for Graphviz
cargo run -- analyze --address mock --output dot

# Output as JSON
cargo run -- analyze --address mock --output json

# Output as table
cargo run -- analyze --address mock --output table
```

## CLI Usage

### Command Structure

```bash
cardano-state-viz <COMMAND> [OPTIONS]
```

### Commands

#### `analyze` - Analyze a Smart Contract

Analyze transactions at a script address and visualize the state machine.

```bash
cardano-state-viz analyze [OPTIONS] --address <ADDRESS>
```

**Options:**

- `--address <ADDRESS>` - Script address to analyze (required)
    - Use `mock` for demo data
    - Or provide a Cardano address (e.g., `addr_test1...`)

- `--source <SOURCE>` - Data source (default: `mock`)
    - `mock` - Use built-in mock vesting contract data
    - `blockfrost` - Query Blockfrost API (requires API key in config)
    - `node` - Query local Cardano node (not yet implemented)

- `--output <FORMAT>` - Output format (default: `table`)
    - `json` - JSON output with full transaction and datum data
    - `table` - Formatted table view
    - `dot` - Graphviz DOT format for graph visualization
    - `tui` - Interactive terminal UI (recommended)

- `--schema <PATH>` - Path to contract schema file (optional)
    - Example: `--schema schemas/vesting.toml`

- `--no-cache` - Disable caching of fetched data (enabled by default)

- `--cache-ttl <DURATION>` - Cache Time-To-Live (default: `3600s`)
    - Supports units: `ms`, `s`, `m`, `h`, `d` (e.g., `1h`, `30m`)

- `--max-transactions <N>` - Limit the number of transactions to fetch (optional)

**Examples:**

```bash
# Interactive TUI with mock data
cargo run -- analyze --address mock --output tui

# Export state graph to PNG via Graphviz
cargo run -- analyze --address mock --output dot

# Analyze with custom schema and increased cache time
cargo run -- analyze --address mock --schema schemas/vesting.toml --cache-ttl 24h --output tui

# JSON output for programmatic processing
cargo run -- analyze --address mock --output json | jq '.transactions | length'
```

#### `watch` - Watch for New Transactions

Watch a script address for new transactions in real-time.

```bash
cardano-state-viz watch [OPTIONS] --address <ADDRESS>
```

**Options:**

- `--address <ADDRESS>` - Script address to watch (required)
- `--interval <DURATION>` - Polling interval (default: `30s`)
- `--max-transactions <N>` - Limit initial fetch size

#### `schema-validate` - Validate a Contract Schema

Validate the structure and syntax of a contract schema file.

```bash
cardano-state-viz schema-validate <SCHEMA_PATH>
```

## TUI (Terminal User Interface)

Launch the interactive TUI for the best visualization experience:

```bash
cargo run -- analyze --address mock --output tui
```

### TUI Views

The TUI has six different views you can switch between:

1. **Graph Overview** - List of all states sorted by block/slot
2. **State Detail** - Detailed view of selected state with transitions
3. **Transaction List** - All transactions affecting the contract
4. **Datum Inspector** - Hex and decoded views of datum data
5. **Pattern Analysis** - Analysis of the contract's state machine structure
6. **Help** - Keyboard shortcuts and legend

### Keyboard Shortcuts

#### Navigation

- `↑/↓` - Navigate through items (context-aware)
- `Enter` - Open detail view (from lists)
- `Esc` - Go back to previous view

#### View Switching

- `g` - Graph overview (state list)
- `d` - State detail view
- `t` - Transaction list view
- `i` - Datum inspector view
- `p` - Pattern analysis view
- `h` or `?` - Help screen
- `Tab` - Cycle through views

#### Datum Inspector

- `x` - Toggle between hex and decoded view

#### General

- `q` - Quit application

### State Color Legend

States are color-coded based on their position in the graph:

- 🔵 **Light Blue (Initial)** - No incoming transitions (newly created)
- 🟡 **Yellow (Active)** - Has both incoming and outgoing transitions
- 🟢 **Green (Completed)** - No outgoing transitions (terminal state)
- 🔴 **Red (Failed)** - Error state
- 🟣 **Magenta (Locked)** - Temporarily locked state
- ⚪ **Gray (Unknown)** - State classification unknown

## Configuration

Configuration file location: `~/.config/cardano-state-viz/config.toml`

### Example Configuration

```toml
[blockfrost]
api_key = "your_api_key_here"
max_retries = 3
retry_delay_ms = 1000

[cache]
enabled = true
ttl = 3600 # seconds

[logging]
level = "info"  # trace, debug, info, warn, error
```

### Environment Variables

- `RUST_LOG` - Set log level (overrides config)
- `BLOCKFROST_API_KEY` - Blockfrost API key (overrides config)

## Output Formats

### JSON

Full transaction and datum data in JSON format:

```json
{
  "transactions": [
    {
      "hash": "tx1",
      "block": 100,
      "slot": 1000,
      "inputs": [],
      "outputs": [
        {
          "address": "addr_test1...",
          "amount": [
            {
              "unit": "lovelace",
              "quantity": "10000000"
            }
          ],
          "datum": {
            "hash": "datum1",
            "raw_cbor": [
              1,
              2,
              3
            ],
            "parsed": {
              ...
            }
          }
        }
      ]
    }
  ]
}
```

### Table

Human-readable table format:

```
Transaction: tx1
Block: 100 | Slot: 1000
Inputs: 0
Outputs: 1
  [0] addr_test1... | 10.0 ADA
      Datum: datum1
```

### DOT (Graphviz)

Graph visualization format for Graphviz:

```dot
digraph StateGraph {
  rankdir=LR;
  node [shape=box, style=filled];

  tx1_0 [label="tx1#0\nBlock: 100", fillcolor="lightblue"];
  tx2_0 [label="tx2#0\nBlock: 200", fillcolor="green"];

  tx1_0 -> tx2_0 [label="Spend"];
}
```

## Resources

- [Cardano Documentation](https://docs.cardano.org/)
- [Plutus Documentation](https://plutus.readthedocs.io/)
- [PlutusData Specification](https://github.com/input-output-hk/plutus)
- [Blockfrost API](https://blockfrost.io/)

---

**Built with Rust** 🦀 | **Powered by Ratatui** 🐭
