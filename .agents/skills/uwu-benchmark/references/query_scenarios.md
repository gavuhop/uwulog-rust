# 7 Standard Benchmark Query Scenarios

The `uwulog-rust` benchmark suite evaluates query latency and throughput across 7 standardized query scenarios representing varying levels of complexity:

| # | Scenario | Example Query | Technical Description | Processing Characteristics |
| :--- | :--- | :--- | :--- | :--- |
| **01** | `01_empty_query` | `*(empty)*` | Fast-path in-memory columnar slice retrieval | Direct sequential slice retrieval without activating AST parsing or filter evaluation |
| **02** | `02_substring_match` | `database connection timeout` | Full-text substring search | Case-insensitive full-text substring scan across message and standard attributes |
| **03** | `03_exact_field_match` | `level:error` | Exact matching on Canonical Field | Direct comparison against standard fields with $O(1)$ byte comparison cost |
| **04** | `04_regex_match` | `tag:~"^sys\.[a-z]+" message:~"user_\d{4}"` | Multi-field Regular Expression matching | Parallel multi-column regex compilation and pattern evaluation |
| **05** | `05_numeric_range` | `latency:>=500 duration:100..800` | Numeric range and threshold comparison | Primitive `f64` / `u64` numerical comparisons without string re-parsing |
| **06** | `06_time_window` | `timestamp:now..15m` | Dynamic relative time window | Evaluates pre-computed epoch seconds (`timestamp_secs`) against relative boundary |
| **07** | `07_complex_ast_boolean` | `(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat` | Complex nested Boolean AST tree | Evaluates composite boolean trees combining `AND`, `OR`, parenthetical nesting, and negation (`NOT`) |
