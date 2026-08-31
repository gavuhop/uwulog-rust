package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"math/rand"
	"os"
	"time"
)

func main() {
	var count int
	var rate int
	flag.IntVar(&count, "count", 500000000, "Total number of logs to generate")
	flag.IntVar(&count, "c", 500000000, "Total number of logs to generate (alias)")
	flag.IntVar(&rate, "rate", 0, "Logs per second (0 for unlimited)")
	flag.IntVar(&rate, "r", 0, "Logs per second (alias)")
	flag.Parse()

	countFlag := &count
	rateFlag := &rate
	levels := []string{"TRACE", "DEBUG", "INFO", "NOTICE", "WARN", "ERROR", "FATAL", "CRITICAL", "ALERT", "EMERGENCY"}
	sources := []string{
		"auth_service", "payment_gateway", "image_processor", "db_proxy",
		"frontend_api", "worker_01", "worker_02", "cron_job", "k8s_pod_x72",
		"monitoring_agent", "cache_manager", "email_service",
	}
	methods := []string{"GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"}
	paths := []string{
		"/api/v1/login", "/api/v1/users/profile", "/api/v1/payments/checkout",
		"/api/v1/images/upload", "/health", "/metrics", "/admin/dashboard",
		"/api/v2/products/search", "/api/v1/auth/logout", "/static/css/main.css",
	}
	statusCodes := []int{200, 201, 204, 400, 401, 403, 404, 429, 500, 502, 503, 504}
	ips := []string{
		"192.168.1.5", "10.0.0.10", "172.16.0.45", "127.0.0.1",
		"8.8.8.8", "1.1.1.1", "142.250.190.46", "31.13.71.36",
	}
	userAgents := []string{
		"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
		"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15",
		"PostmanRuntime/7.29.0",
		"curl/7.68.0",
		"Googlebot/2.1 (+http://www.google.com/bot.html)",
	}
	versions := []string{"v1.0.0", "v1.1.0", "v1.2.3", "v2.0.0-beta", "v2.0.1", "v2.1.0-rc1"}
	regions := []string{"us-west-1", "us-west-2", "us-east-1", "eu-central-1", "ap-southeast-1", "ap-northeast-2", "sa-east-1"}
	environments := []string{"production", "staging", "development", "test"}
	clusters := []string{"us-west-cluster-1", "eu-central-cluster-a", "ap-southeast-cluster-k8s", "global-edge-01"}
	messages := []string{
		"User logged in successfully",
		"Connection timeout to server:10.0.1.5",
		"Processing image: \"user_avatar.jpg\" (200x200)",
		"Database query took too long: SELECT * FROM users",
		"Failed to \"validate\" payment token",
		"Retrying connection for the 3rd time...",
		"Disk space low on /var/log",
		"Panic: runtime error: invalid memory address",
		"Log info: database:mysql connection:alive",
		"Incoming request received",
		"Invalid API key provided",
		"Rate limit exceeded for client",
		"Cache miss for key: user_session:8812",
		"Successfully sent email to user@example.com",
		"Worker started processing job: #88213",
		"New user registered: \"john_doe_99\"",
	}
	now := time.Now()
	rand.Seed(time.Now().UnixNano())
	fmt.Fprintf(os.Stderr, "Generating %d logs (Rate: %d logs/sec) to stdout...\n", *countFlag, *rateFlag)
	var ticker *time.Ticker
	if *rateFlag > 0 {
		interval := time.Second / time.Duration(*rateFlag)
		ticker = time.NewTicker(interval)
		defer ticker.Stop()
	}
	encoder := json.NewEncoder(os.Stdout)
	for i := 0; i < *countFlag; i++ {
		if ticker != nil {
			<-ticker.C
		}
		var ts time.Time
		if *rateFlag > 0 {
			ts = time.Now()
		} else {
			if rand.Float32() < 0.2 {
				burstStart := now.Add(time.Duration(-rand.Intn(48*60)) * time.Minute)
				ts = burstStart.Add(time.Duration(rand.Intn(60)) * time.Second)
			} else {
				ts = now.Add(time.Duration(-rand.Intn(48*60)) * time.Minute)
			}
		}
		entry := make(map[string]interface{})
		entry["timestamp"] = ts.Format(time.RFC3339)
		entry["level"] = levels[rand.Intn(len(levels))]
		entry["source"] = sources[rand.Intn(len(sources))]
		entry["message"] = messages[rand.Intn(len(messages))]
		entry["trace_id"] = fmt.Sprintf("tr-%x", rand.Intn(1000000000))
		if rand.Float32() < 0.6 {
			entry["user_id"] = fmt.Sprintf("user_%d", rand.Intn(500))
		}
		if rand.Float32() < 0.4 {
			entry["latency"] = rand.Intn(2000)
			if rand.Float32() < 0.1 {
				entry["latency"] = rand.Float64() * 1000.0
			}
		}
		if rand.Float32() < 0.5 {
			entry["ip"] = ips[rand.Intn(len(ips))]
			entry["method"] = methods[rand.Intn(len(methods))]
			entry["path"] = paths[rand.Intn(len(paths))]
			entry["status"] = statusCodes[rand.Intn(len(statusCodes))]
			if rand.Float32() < 0.3 {
				entry["user_agent"] = userAgents[rand.Intn(len(userAgents))]
			}
		}
		if rand.Float32() < 0.15 {
			meta := make(map[string]interface{})
			meta["version"] = versions[rand.Intn(len(versions))]
			meta["region"] = regions[rand.Intn(len(regions))]
			meta["instance_id"] = fmt.Sprintf("i-%x", rand.Intn(0xffffff))
			meta["system"] = map[string]interface{}{
				"env": environments[rand.Intn(len(environments))],
				"infra": map[string]interface{}{
					"cluster": clusters[rand.Intn(len(clusters))],
					"zone":    fmt.Sprintf("%s-%c", meta["region"], 'a'+rand.Intn(3)),
				},
			}
			entry["metadata"] = meta
		}
		// Edge cases
		if i == 500 {
			entry["message"] = "Special characters: 🚀 🔥 🌈 \t \"quoted\" 'single' \\backslashes\\ \n newline"
		}
		if i == 1000 {
			entry["stack_trace"] = "main.go:42\n  at processRequest()\n  at handleLogin()\n  at auth_service.go:121"
			entry["level"] = "ERROR"
		}
		_ = encoder.Encode(entry)
		_ = os.Stdout.Sync() // Force flushing the OS pipe buffer immediately
	}
	fmt.Fprintln(os.Stderr, "\nDone!")
}
