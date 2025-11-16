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
- `--strict-typing` - Differentiate int/float types (default: true)
- `-r, --repeat-threshold <N>` - Min repeats to summarize (default: 2)

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

### `strict_typing` (default: true)

- `true`: Treats int and float as different types (more precise)
- `false`: All primitives are generic "values" (more compression)

### `repeat_threshold` (default: 2)

- `1`: Aggressive - summarize after 1 repeat
- `2`: Balanced - summarize after 2 repeats (recommended)
- `3+`: Conservative - require more repeats before summarizing

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
