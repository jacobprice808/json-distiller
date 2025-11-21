# JSON Distiller

**Reverse engineer arbitrary JSON structures for LLM analysis without breaking context.**

JSON Distiller identifies structural patterns in large JSON payloads and compresses them by **99%+** while preserving complete structural information. Perfect for understanding APIs, datasets, and complex JSON when you care about **structure and patterns**, not individual content values.

## Why This Exists

Large Language Models have limited context windows. When analyzing a 10MB API response with thousands of similar objects, you don't need to see every object—you need to understand the **structure**. JSON Distiller solves this by:

- **Identifying unique structures** in your JSON (not content)
- **Showing one example** of each unique structure
- **Summarizing repetition** with count and pattern information
- **Preserving 100% of structural information** while removing redundant content

### Information Density

**Before:** 9.4 MB of repetitive API data
**After:** 84 KB with full structural clarity
**Compression:** 99.1%
**Time:** 0.13 seconds

## Installation

### From Source

```bash
git clone https://github.com/jacobprice808/json-distiller.git
cd json-distiller
cargo build --release
```

The binary will be at `target/release/json-distiller`.

### Install to PATH

```bash
cargo install --path .
```

This installs to `~/.cargo/bin/json-distiller` (ensure `~/.cargo/bin` is in your PATH).

## Usage

### Command Line

```bash
# Basic usage
json-distiller input.json

# Specify output file
json-distiller input.json -o output.json

# Adjust options
json-distiller input.json --strict-typing=false -r 1
```

**Options:**
- `--strict-typing=<bool>` - Differentiate int/float types (default: true)
- `--position-dependent=<bool>` - Control example display across nesting levels (default: true)
- `-r, --repeat-threshold <N>` - Min repeats to summarize (default: 1)

### As MCP Server (for Claude Code/Desktop)

Add to `.mcp.json` (Claude Code) or `claude_desktop_config.json` (Claude Desktop):

```json
{
  "mcpServers": {
    "json-distiller": {
      "command": "/path/to/json-distiller",
      "args": ["--mcp-server"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

Claude can then use `distill_json_content` tool to analyze JSON structures.

## Example

### Input: API Response (3,831 similar objects)

```json
{
  "data": [
    {
      "type": "user",
      "id": "1",
      "attributes": {
        "name": "Alice",
        "email": "alice@example.com",
        "age": 30,
        "active": true
      }
    },
    {
      "type": "user",
      "id": "2",
      "attributes": {
        "name": "Bob",
        "email": "bob@example.com",
        "age": 25,
        "active": true
      }
    }
    // ... 3,829 more similar objects
  ]
}
```

### Output: Structural Analysis

```json
{
  "distilled_data": {
    "data": [
      {
        "_structure_hash": "a1b2c3d4",
        "type": "user",
        "id": "1",
        "attributes": {
          "name": "Alice",
          "email": "alice@example.com",
          "age": 30,
          "active": true
        }
      },
      {
        "item_count": 3830,
        "summarized_pattern": "a1b2c3d4(x3830)"
      }
    ]
  }
}
```

**Key insight:** All 3,831 objects share the same structure (hash `a1b2c3d4`). You see one example plus a summary.

## How It Works

1. **Deep Structure Detection** - Recursively analyzes JSON to build structure signatures
2. **Pattern Identification** - Groups objects by identical structure (not values)
3. **Smart Summarization** - Shows first example of each unique structure
4. **Pattern Notation** - Expresses repetition concisely (e.g., `hash(x10)`)

### What Counts as "Same Structure"?

**Same structure** (will be summarized):
```json
{"id": 1, "name": "Alice", "age": 30}
{"id": 2, "name": "Bob", "age": 25}
```

**Different structures** (both shown):
```json
{"id": 1, "name": "Alice", "age": 30}
{"id": 2, "name": "Bob"}  // Missing 'age' field
```

With `strict_typing=true` (default):
```json
{"score": 42}   // int - different structure
{"score": 42.0} // float - different structure
```

## Use Cases

### 1. API Response Analysis
Understand the structure of paginated API responses with thousands of similar records.

### 2. Dataset Exploration
Quickly identify structural patterns in large JSON datasets without loading everything into context.

### 3. Schema Discovery
Reverse engineer implicit schemas from untyped JSON data.

### 4. LLM Context Optimization
Analyze large JSON within LLM context limits while preserving structural insights.

## Performance

- **Speed:** ~72 MB/second
- **Memory:** Efficient streaming processing
- **Compression:** 99%+ on repetitive data
- **Validated:** 100% match with Python reference implementation

Tested with 23 edge cases including: empty structures, deep nesting, unicode, mixed types, 10k+ objects, and more.

## Configuration

All configuration options work in both CLI and MCP modes.

### `--strict-typing` (default: `true`)

Controls whether integers and floats are treated as distinct structure types.

**When `true` (recommended for most use cases):**
- `{"score": 42}` and `{"score": 42.0}` have **different** structures
- More precise structure detection
- Better for understanding typed data

**When `false`:**
- All number types treated as generic "number"
- Higher compression on numeric data
- Use when type precision doesn't matter

**Example:**
```bash
json-distiller data.json --strict-typing=false
```

### `--position-dependent` (default: `true`)

Controls how structure examples are displayed across different nesting levels.

**When `true` (depth-aware mode):**
- Shows examples independently at each nesting level
- Same structure appearing at different depths will show separate examples
- More predictable: you see examples for each level you're analyzing
- **Use when:** You need to see structure at every nesting level

**When `false` (global mode):**
- Shows examples only at the shallowest occurrence
- Same structure at deeper levels is fully summarized
- More concise output
- **Use when:** You want minimal examples, maximum compression

**Example scenario:**
Given nested sports data where "league" structure appears both at top-level and within sections:

- `position-dependent=true`: Shows league example at top level AND within sections
- `position-dependent=false`: Shows league example only at top level, summarizes nested occurrences

**Example:**
```bash
json-distiller data.json --position-dependent=false
```

### `-r, --repeat-threshold` (default: `1`)

Controls internal pattern formatting in summaries. This affects how repetition patterns are displayed.

**Values:**
- `1`: Most aggressive - summarize all patterns
- `2`: Balanced - require 2+ repeats (recommended)
- `3+`: Conservative - require more evidence before pattern summarization

**Example:**
```bash
json-distiller data.json -r 2
```

### MCP-Specific Parameters

When using JSON Distiller as an MCP server with Claude, additional parameters are available:

#### `strict_typing` (boolean, default: `true`)
Same as CLI option above.

#### `position_dependent` (boolean, default: `true`)
Same as CLI option above.

#### `repeat_threshold` (integer, default: `2`)
Same as CLI option above.

**MCP Example:**
```json
{
  "json_string": "{\"data\": [...]}",
  "strict_typing": true,
  "position_dependent": false,
  "repeat_threshold": 2
}
```

### Advanced: GHOST Mode (Python only)

**Note:** GHOST mode is currently only available in the Python reference implementation.

GHOST mode shows value ranges for primitive fields instead of just structure:
- Single unique value: shown as-is
- Multiple values (up to N): shown as `[value1, value2, ...]`
- More than N values: shown with indicator `... (and X more unique values)`

This mode is experimental and not yet available in the Rust CLI.

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Repository Structure

```
json-distiller/
├── src/           # Rust source code
├── tests/         # Test suite
├── examples/      # Example data and outputs
├── docs/          # Documentation
└── README.md      # This file
```

## Documentation

- **[Performance & Options](docs/PERFORMANCE_AND_OPTIONS.md)** - Detailed parameter guide
- **[MCP Protocol](docs/MCP_PROTOCOL_GUIDE.md)** - Integration details
- **[Validation Report](docs/VALIDATION_REPORT.md)** - Test results

## Contributing

Issues and pull requests welcome! This tool was built to help LLMs understand JSON structures efficiently.

---

**Built for clarity.** When you need to understand structure, not wade through content.
