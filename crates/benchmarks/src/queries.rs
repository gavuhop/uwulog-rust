//! Danh mục các câu truy vấn benchmark chuẩn hóa theo 7 cấp độ phức tạp

#[derive(Debug, Clone)]
pub struct BenchmarkQuery {
    pub name: &'static str,
    pub query: &'static str,
    pub description: &'static str,
}

pub const BENCHMARK_QUERIES: &[BenchmarkQuery] = &[
    BenchmarkQuery {
        name: "01_empty_query",
        query: "",
        description: "Fast-path in-memory RingBuffer retrieval without AST evaluation",
    },
    BenchmarkQuery {
        name: "02_substring_match",
        query: "database connection timeout",
        description: "Full-text case-insensitive substring search across all fields",
    },
    BenchmarkQuery {
        name: "03_exact_field_match",
        query: "level:error",
        description: "Direct canonical field exact matching on LogLevel",
    },
    BenchmarkQuery {
        name: "04_regex_match",
        query: r#"tag:~"^sys\.[a-z]+" message:~"user_\d{4}""#,
        description: "Multi-field Regular Expression matching",
    },
    BenchmarkQuery {
        name: "05_numeric_range",
        query: "latency:>=500 duration:100..800",
        description: "Multi-field Numeric Comparison (>=) and Order-independent Range (..)",
    },
    BenchmarkQuery {
        name: "06_time_window",
        query: "timestamp:now..15m",
        description: "Dynamic relative time window calculation based on latest log timestamp",
    },
    BenchmarkQuery {
        name: "07_complex_ast_boolean",
        query: r#"(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat"#,
        description: "Complex Nested AST Boolean logic with AND, OR, grouping, and negation",
    },
];
