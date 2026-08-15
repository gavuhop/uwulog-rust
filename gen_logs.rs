//! Rust Log Generator for uwulog-rust
//! Generates diverse, realistic structured JSON (and text) logs to stdout.
//! 
//! Build & Run:
//!   rustc -O gen_logs.rs
//!   ./gen_logs.exe --rate 30
//!   ./gen_logs.exe --count 100000 --rate 0
//! 
//! Or pipe directly into uwulog-gui / uwulog-tui:
//!   ./gen_logs.exe --rate 20 | cargo run --bin uwulog-gui

use std::env;
use std::io::{self, BufWriter, Write};
use std::process;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// --- Fast Pseudo-Random Generator (Zero Dependency) ---
struct FastRng {
    state: u64,
}

impl FastRng {
    fn new(seed: u64) -> Self {
        let s = if seed == 0 { 0x853c49e6748fea9b } else { seed };
        Self { state: s }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    #[inline]
    fn gen_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        min + (self.next_u64() as usize % (max - min))
    }

    #[inline]
    fn gen_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1 << 24) as f32
    }

    #[inline]
    fn choose<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        &slice[self.gen_range(0, slice.len())]
    }
}

// --- Fast RFC 3339 / ISO 8601 Timestamp Formatter ---
fn format_rfc3339(time: SystemTime) -> String {
    let dur = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let total_secs = dur.as_secs();
    let millis = dur.subsec_millis();

    let days = (total_secs / 86400) as i64;
    let rem_secs = (total_secs % 86400) as u32;

    let hour = rem_secs / 3600;
    let minute = (rem_secs % 3600) / 60;
    let second = rem_secs % 60;

    // Howard Hinnant's algorithm for civil date
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 {
        y += 1;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y, m, d, hour, minute, second, millis
    )
}

// --- Sample Log Fields Data ---
const LEVELS: &[&str] = &[
    "TRACE", "DEBUG", "INFO", "NOTICE", "WARN", "ERROR", "FATAL", "CRITICAL", "ALERT", "EMERGENCY",
];

const SOURCES: &[&str] = &[
    "auth_service",
    "payment_gateway",
    "image_processor",
    "db_proxy",
    "frontend_api",
    "worker_01",
    "worker_02",
    "cron_job",
    "k8s_pod_x72",
    "monitoring_agent",
    "cache_manager",
    "email_service",
];

const HTTP_METHODS: &[&str] = &["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"];

const PATHS: &[&str] = &[
    "/api/v1/login",
    "/api/v1/users/profile",
    "/api/v1/payments/checkout",
    "/api/v1/images/upload",
    "/healthz",
    "/metrics",
    "/admin/dashboard",
    "/api/v2/products/search",
    "/api/v1/auth/logout",
    "/static/css/main.css",
    "/ws/notifications",
    "/api/v3/orders/bulk",
];

const STATUS_CODES: &[u16] = &[200, 201, 204, 400, 401, 403, 404, 429, 500, 502, 503, 504];

const IPS: &[&str] = &[
    "192.168.1.5",
    "10.0.0.10",
    "172.16.0.45",
    "127.0.0.1",
    "8.8.8.8",
    "1.1.1.1",
    "142.250.190.46",
    "31.13.71.36",
    "104.244.42.1",
];

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15",
    "PostmanRuntime/7.29.0",
    "curl/7.68.0",
    "Googlebot/2.1 (+http://www.google.com/bot.html)",
    "uwulog-agent/1.0",
];

const VERSIONS: &[&str] = &["v1.0.0", "v1.1.0", "v1.2.3", "v2.0.0-beta", "v2.0.1", "v2.1.0-rc1"];
const REGIONS: &[&str] = &["us-west-1", "us-west-2", "us-east-1", "eu-central-1", "ap-southeast-1", "ap-northeast-2", "sa-east-1"];
const ENVIRONMENTS: &[&str] = &["production", "staging", "development", "test"];
const CLUSTERS: &[&str] = &["us-west-cluster-1", "eu-central-cluster-a", "ap-southeast-cluster-k8s", "global-edge-01"];

const MESSAGES: &[&str] = &[
    "User logged in successfully",
    "Connection timeout to server:10.0.1.5",
    "Processing image: \"user_avatar.jpg\" (200x200)",
    "Database query took too long: SELECT * FROM users",
    "Failed to validate payment token",
    "Retrying connection for the 3rd time...",
    "Disk space low on /var/log",
    "Panic: runtime error: invalid memory address or nil pointer dereference",
    "Log info: database:mysql connection:alive pool_size:16",
    "Incoming HTTP request received",
    "Invalid API key provided in Authorization header",
    "Rate limit exceeded for client IP",
    "Cache miss for key: user_session:8812",
    "Successfully sent email notification to user@example.com",
    "Worker started processing async background job: #88213",
    "New user registered successfully",
    "Circuit breaker OPEN for payment gateway service",
    "SSL certificate validation succeeded",
];

// Helper to escape JSON strings safely
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn print_help() {
    eprintln!(
        r#"Rust Log Generator (uwulog-rust)

USAGE:
    gen_logs [OPTIONS]

OPTIONS:
    -c, --count <NUMBER>   Total number of logs to generate (default: 500000000)
    -r, --rate <NUMBER>    Logs per second rate limit (0 for unlimited, default: 0)
    -f, --format <FORMAT>  Log format: 'json' (default), 'text', or 'mixed'
    -h, --help             Print help information
"#
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut count: u64 = 500_000_000;
    let mut rate: u64 = 0;
    let mut format_mode = "json".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-c" | "--count" => {
                if i + 1 < args.len() {
                    count = args[i + 1].parse().unwrap_or(count);
                    i += 1;
                }
            }
            "-r" | "--rate" => {
                if i + 1 < args.len() {
                    rate = args[i + 1].parse().unwrap_or(rate);
                    i += 1;
                }
            }
            "-f" | "--format" => {
                if i + 1 < args.len() {
                    format_mode = args[i + 1].to_lowercase();
                    i += 1;
                }
            }
            "-h" | "--help" => {
                print_help();
                process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x123456789ABCDEF0);
    let mut rng = FastRng::new(seed);

    eprintln!(
        "Generating {} logs (Rate: {} logs/sec, Format: {}) to stdout...",
        count, rate, format_mode
    );

    let stdout = io::stdout();
    let mut writer = BufWriter::with_capacity(64 * 1024, stdout.lock());

    let interval = if rate > 0 {
        Some(Duration::from_secs_f64(1.0 / rate as f64))
    } else {
        None
    };

    let start_time = Instant::now();
    let mut next_tick = Instant::now();

    for idx in 0..count {
        if let Some(step) = interval {
            let now = Instant::now();
            if now < next_tick {
                thread::sleep(next_tick - now);
            }
            next_tick += step;
        }

        let now_sys = SystemTime::now();
        let ts_str = format_rfc3339(now_sys);
        let level = *rng.choose(LEVELS);
        let source = *rng.choose(SOURCES);
        let message = *rng.choose(MESSAGES);
        let trace_id = format!("tr-{:08x}", rng.next_u32());

        // Select format
        let is_json = if format_mode == "mixed" {
            rng.gen_f32() > 0.3
        } else {
            format_mode != "text"
        };

        if is_json {
            // Build dynamic JSON log with diverse fields
            let mut json_obj = String::with_capacity(512);
            json_obj.push_str("{\"timestamp\":\"");
            json_obj.push_str(&ts_str);
            json_obj.push_str("\",\"level\":\"");
            json_obj.push_str(level);
            json_obj.push_str("\",\"source\":\"");
            json_obj.push_str(source);
            json_obj.push_str("\",\"message\":\"");
            json_obj.push_str(&escape_json(message));
            json_obj.push_str("\",\"trace_id\":\"");
            json_obj.push_str(&trace_id);
            json_obj.push('"');

            // Optional User ID (60% chance)
            if rng.gen_f32() < 0.6 {
                let user_id = rng.gen_range(1, 500);
                json_obj.push_str(&format!(",\"user_id\":\"user_{}\"", user_id));
            }

            // Optional Latency (40% chance)
            if rng.gen_f32() < 0.4 {
                if rng.gen_f32() < 0.2 {
                    let lat = (rng.next_u32() % 2000) as f32 + rng.gen_f32();
                    json_obj.push_str(&format!(",\"latency\":{:.2}", lat));
                } else {
                    let lat = rng.gen_range(5, 1500);
                    json_obj.push_str(&format!(",\"latency\":{}", lat));
                }
            }

            // Optional HTTP Request fields (50% chance)
            if rng.gen_f32() < 0.5 {
                let ip = *rng.choose(IPS);
                let method = *rng.choose(HTTP_METHODS);
                let path = *rng.choose(PATHS);
                let status = *rng.choose(STATUS_CODES);

                json_obj.push_str(&format!(
                    ",\"ip\":\"{}\",\"method\":\"{}\",\"path\":\"{}\",\"status\":{}",
                    ip, method, path, status
                ));

                if rng.gen_f32() < 0.3 {
                    let ua = *rng.choose(USER_AGENTS);
                    json_obj.push_str(&format!(",\"user_agent\":\"{}\"", escape_json(ua)));
                }
            }

            // Optional Nested Metadata (20% chance)
            if rng.gen_f32() < 0.2 {
                let ver = *rng.choose(VERSIONS);
                let reg = *rng.choose(REGIONS);
                let env = *rng.choose(ENVIRONMENTS);
                let cluster = *rng.choose(CLUSTERS);
                let inst_id = format!("i-{:06x}", rng.next_u32() & 0xFFFFFF);
                let zone = format!("{}-{}", reg, (b'a' + (rng.gen_range(0, 3) as u8)) as char);

                json_obj.push_str(&format!(
                    ",\"metadata\":{{\"version\":\"{}\",\"region\":\"{}\",\"instance_id\":\"{}\",\"system\":{{\"env\":\"{}\",\"infra\":{{\"cluster\":\"{}\",\"zone\":\"{}\"}}}}}}",
                    ver, reg, inst_id, env, cluster, zone
                ));
            }

            // Special Edge Cases for testing parsing
            if idx == 500 {
                json_obj.push_str(",\"special_chars\":\"🚀 🔥 🌈 \\t \\\"quoted\\\" 'single' \\\\backslashes\\\\ \\n newline\"");
            } else if idx == 1000 || level == "FATAL" || (level == "ERROR" && rng.gen_f32() < 0.1) {
                json_obj.push_str(",\"stack_trace\":\"main.rs:42\\n  at process_request()\\n  at handle_login()\\n  at auth_service.rs:121\"");
            }

            json_obj.push_str("}\n");
            let _ = writer.write_all(json_obj.as_bytes());
        } else {
            // Unstructured Text / Common Log Format
            let ip = *rng.choose(IPS);
            let method = *rng.choose(HTTP_METHODS);
            let path = *rng.choose(PATHS);
            let status = *rng.choose(STATUS_CODES);
            let lat = rng.gen_range(5, 500);

            let log_line = format!(
                "{} [{}] {} {} - \"{} {}\" {} - {} ({}ms) trace_id={}\n",
                ts_str, level, source, ip, method, path, status, message, lat, trace_id
            );
            let _ = writer.write_all(log_line.as_bytes());
        }

        // Flush immediately if rate-limited so consumer receives live stream
        if rate > 0 {
            let _ = writer.flush();
        }
    }

    let _ = writer.flush();
    let elapsed = start_time.elapsed();
    eprintln!(
        "\nFinished generating {} logs in {:.2?} ({:.0} logs/sec)",
        count,
        elapsed,
        count as f64 / elapsed.as_secs_f64().max(0.0001)
    );
}
