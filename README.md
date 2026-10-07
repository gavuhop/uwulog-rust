# Uwu Log Viewer

A blazing-fast, cross-platform real-time log viewer and workspace monitor powered by an in-memory Apache Arrow columnar engine.

## Advanced Filter Syntax

Uwu Log supports complex queries to help you find exactly what you're looking for.

### Filter Patterns

| Syntax                | Example              | Description                                              |
| --------------------- | -------------------- | -------------------------------------------------------- |
| **Field Search**      | `level:error`        | Field contains "error" (case-insensitive)                |
| **Logic OR**          | `tag:Audio\|Video`   | Field contains "Audio" **OR** "Video"                    |
| **Exact Match**       | `level=ERROR`        | Exact match (case-insensitive)                           |
| **Regex**             | `tag:~^sys`          | Field matches **regular expression**                     |
| **Negation (Global)** | `-timeout`           | Search for items **NOT** containing "timeout"            |
| **Negation (Field)**  | `level:-error`       | Field **NOT** containing "error"                         |
| **Negated Regex**     | `tag:-~^sys`         | Field **NOT** matching regex                             |
| **Numeric Cmp**       | `latency:<=500`      | Supports `>`, `<`, `>=`, `<=`                            |
| **Numeric Range**     | `heal:200..500`      | Logic is order-independent (`200..500` == `500..200`)    |
| **Time Range**        | `timestamp:now..10m` | Show logs from the last 10 minutes                       |
| **Text Search**       | `timeout`            | Search in message, source, and extras                    |
| **Phrases**           | `"login failed"`     | Match exact phrase with spaces                           |
| **Boolean**           | `AND`, `OR`, `NOT`   | Standard boolean operators                               |
| **Grouping**          | `( )`                | Change precedence (e.g., `lvl:err AND (tag:A OR tag:B)`) |

### Smart Time Selection

Combined with the keyword `now` (which refers to the **latest log**), you can quickly filter by time window.

| Shorthand            | Meaning         | Equivalent                    |
| -------------------- | --------------- | ----------------------------- |
| `timestamp:now..10m` | Last 10 minutes | Log window: 10m ago to latest |
| `timestamp:1h..30m`  | Specific window | From 1h ago to 30m ago        |
| `timestamp:>15m`     | Last 15 minutes | Newer than 15m ago            |
| `timestamp:<15m`     | Older than 15m  | Prior to 15m ago              |

**Time Units**: `m` (min), `h` (hour), `d` (day), `w` (week), `M` (month), `y` (year).

### Complex Query Examples

- `(level:error OR level:warn) timestamp:now..30m`
- `timestamp:now..15m -tag:health "database error"`
- `source~:^sys level:error latency:>=1000`
- `timestamp:now..2h message:auth|login`

## Features

- **Blazing Fast**: Filtering handled by Rust for near-instant results even with large logs.
- **Data-Driven `now`**: Time filters automatically refer to the latest log, making them work perfectly on both live and static log files.
- **Smart Highlighting**: Automatically identifies levels, timestamps, and sources.
- **Context Awareness**: Click source tags to jump directly to the code.
- **Intelligent Prompt**: The `>` sign changes color to reflect the process state.
- **Process Management**: `Ctrl+C` copies selected text or stops the current command if nothing is selected.
