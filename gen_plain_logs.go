package main

import (
	"errors"
	"flag"
	"fmt"
	"math/rand"
	"net/http"
	"os"
	"time"

	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/recover"
	"go.uber.org/zap"
	"go.uber.org/zap/zapcore"
)

func main() {
	countFlag := flag.Int("count", 500000000, "Total number of requests/logs to generate")
	rateFlag := flag.Int("rate", 15, "Requests per second (0 for unlimited)")
	portFlag := flag.Int("port", 3000, "Port for Fiber web server")
	jsonFlag := flag.Bool("json", false, "Output logs in structured JSON format (via Uber Zap)")
	stdoutFlag := flag.Bool("stdout", false, "Output logs in human-readable plain text with ANSI colors")
	formatFlag := flag.String("format", "", "Output format: 'json' or 'stdout'/'plain'")
	flag.Parse()

	// Quyết định chế độ xuất log: JSON hay Plain stdout
	isJSON := false
	if *jsonFlag {
		isJSON = true
	} else if *stdoutFlag {
		isJSON = false
	} else if *formatFlag == "json" {
		isJSON = true
	}

	rand.Seed(time.Now().UnixNano())

	// Khởi tạo Zap Logger nếu ở chế độ JSON
	var zapLogger *zap.Logger
	if isJSON {
		encoderConfig := zap.NewProductionEncoderConfig()
		encoderConfig.TimeKey = "timestamp"
		encoderConfig.EncodeTime = zapcore.ISO8601TimeEncoder
		encoderConfig.EncodeLevel = zapcore.CapitalLevelEncoder
		core := zapcore.NewCore(
			zapcore.NewJSONEncoder(encoderConfig),
			zapcore.AddSync(os.Stdout),
			zapcore.DebugLevel,
		)
		zapLogger = zap.New(core)
		defer zapLogger.Sync()
	}

	// 1. Khởi tạo ứng dụng Go Fiber với ProxyHeader để nhận diện IP mô phỏng
	app := fiber.New(fiber.Config{
		AppName:               "UwULog Realistic Fiber Simulator v1.0",
		DisableStartupMessage: true,
		ProxyHeader:           fiber.HeaderXForwardedFor,
	})

	// 2. Middleware Logger của Fiber hỗ trợ cả 2 chế độ: JSON (Zap) và Plain (ANSI Colors)
	app.Use(func(c *fiber.Ctx) error {
		start := time.Now()
		err := c.Next()
		latency := time.Since(start)
		latencyMs := float64(latency.Microseconds()) / 1000.0

		status := c.Response().StatusCode()
		var errMsg string

		if err != nil {
			var e *fiber.Error
			if errors.As(err, &e) {
				status = e.Code
				errMsg = e.Message
			} else {
				status = fiber.StatusInternalServerError
				errMsg = err.Error()
			}
		}

		traceID := c.Get("X-Request-ID")
		if traceID == "" {
			traceID = fmt.Sprintf("tr-%08x", rand.Uint32())
		}

		if isJSON {
			// Xuất log cấu trúc chuẩn JSON bằng Uber Zap
			fields := []zap.Field{
				zap.Int("status", status),
				zap.String("method", c.Method()),
				zap.String("path", c.Path()),
				zap.String("ip", c.IP()),
				zap.Float64("latency_ms", latencyMs),
				zap.String("trace_id", traceID),
				zap.String("user_agent", c.Get("User-Agent")),
			}

			if loc := c.GetRespHeader("Location"); loc != "" {
				fields = append(fields, zap.String("redirect_to", loc))
			}

			if errMsg != "" {
				fields = append(fields, zap.String("error", errMsg))
			}

			switch {
			case status >= 500:
				zapLogger.Error("HTTP request server error", fields...)
			case status >= 400:
				zapLogger.Warn("HTTP request client error", fields...)
			case status >= 300:
				zapLogger.Info("HTTP request redirected", fields...)
			default:
				zapLogger.Info("HTTP request completed", fields...)
			}
		} else {
			// Xuất log văn bản thuần kèm mã màu ANSI
			var statusColor string
			switch {
			case status >= 100 && status < 200:
				statusColor = "\x1b[34m" // Blue (Informational 1xx)
			case status >= 200 && status < 300:
				statusColor = "\x1b[32m" // Green (Success 2xx)
			case status >= 300 && status < 400:
				statusColor = "\x1b[36m" // Cyan (Redirection / Cache 3xx)
			case status >= 400 && status < 500:
				statusColor = "\x1b[33m" // Yellow / Amber (Client Error 4xx)
			default:
				statusColor = "\x1b[31m" // Red (Server Error 5xx)
			}

			// Mã màu ANSI theo HTTP Method
			var methodColor string
			switch c.Method() {
			case "GET":
				methodColor = "\x1b[36m" // Cyan
			case "POST":
				methodColor = "\x1b[32m" // Green
			case "PUT":
				methodColor = "\x1b[33m" // Yellow
			case "DELETE":
				methodColor = "\x1b[31m" // Red
			case "PATCH":
				methodColor = "\x1b[35m" // Magenta
			case "HEAD":
				methodColor = "\x1b[90m" // Gray
			case "OPTIONS":
				methodColor = "\x1b[34m" // Blue
			default:
				methodColor = "\x1b[37m" // White
			}

			// Chi tiết thông tin lỗi hoặc redirect location
			var extraStr string
			if errMsg != "" {
				if status >= 500 {
					extraStr = fmt.Sprintf(" \x1b[31m%s\x1b[0m", errMsg)
				} else {
					extraStr = fmt.Sprintf(" \x1b[33m%s\x1b[0m", errMsg)
				}
			} else if status >= 300 && status < 400 {
				if loc := c.GetRespHeader("Location"); loc != "" {
					extraStr = fmt.Sprintf(" \x1b[36m-> %s\x1b[0m", loc)
				} else if status == 304 {
					extraStr = " \x1b[90m(Not Modified)\x1b[0m"
				} else {
					extraStr = " -"
				}
			} else if status >= 400 && status < 500 {
				extraStr = fmt.Sprintf(" \x1b[33mCannot %s %s\x1b[0m", c.Method(), c.Path())
			} else {
				extraStr = " -"
			}

			// Định dạng chuỗi log chứa mã màu ANSI y hệt Terminal output của Fiber
			fmt.Printf("[%s] %s%3d\x1b[0m - %12v | %15s | %s%-7s\x1b[0m %s%s\n",
				time.Now().Format("15:04:05"),
				statusColor, status,
				latency,
				c.IP(),
				methodColor, c.Method(),
				c.Path(),
				extraStr,
			)
		}

		return err
	})

	// 3. Middleware Recover bắt Panic và bảo vệ Server
	app.Use(recover.New())

	// 4. Khai báo các Routes thực tế với đa dạng mã Status Codes (2xx, 3xx, 4xx, 5xx)
	// --- 2xx Success ---
	app.Get("/", func(c *fiber.Ctx) error {
		return c.SendString("Hello Fiber! 🚀 Microservice running smoothly.")
	})

	app.Get("/healthz", func(c *fiber.Ctx) error {
		return c.JSON(fiber.Map{
			"status": "healthy",
			"uptime": "24h12m",
		})
	})

	app.Get("/api/v1/users", func(c *fiber.Ctx) error {
		return c.JSON(fiber.Map{
			"users": []string{"alice", "bob", "charlie", "david"},
			"total": 4,
		})
	})

	app.Post("/api/v1/users", func(c *fiber.Ctx) error {
		return c.Status(fiber.StatusCreated).JSON(fiber.Map{
			"id":      rand.Intn(9999) + 1000,
			"created": true,
		})
	})

	app.Put("/api/v1/settings", func(c *fiber.Ctx) error {
		return c.JSON(fiber.Map{"updated": true, "theme": "dark"})
	})

	app.Patch("/api/v1/users/status", func(c *fiber.Ctx) error {
		return c.JSON(fiber.Map{"active": true})
	})

	app.Delete("/api/v1/cache/flush", func(c *fiber.Ctx) error {
		return c.SendStatus(fiber.StatusNoContent)
	})

	app.Head("/api/v1/ping", func(c *fiber.Ctx) error {
		return c.SendStatus(fiber.StatusOK)
	})

	app.Options("/api/v1/cors", func(c *fiber.Ctx) error {
		c.Set("Allow", "GET, POST, PUT, DELETE, OPTIONS")
		return c.SendStatus(fiber.StatusNoContent)
	})

	app.Get("/api/v1/query", func(c *fiber.Ctx) error {
		delay := time.Duration(rand.Intn(40)+5) * time.Millisecond
		time.Sleep(delay)
		return c.SendString("Query completed successfully")
	})

	// --- 3xx Redirection & Cache ---
	app.Get("/v1/old-endpoint", func(c *fiber.Ctx) error {
		return c.Redirect("/api/v1/users", fiber.StatusMovedPermanently) // 301
	})

	app.Get("/docs", func(c *fiber.Ctx) error {
		return c.Redirect("/docs/v2/getting-started", fiber.StatusFound) // 302
	})

	app.Get("/static/app.bundle.js", func(c *fiber.Ctx) error {
		return c.SendStatus(fiber.StatusNotModified) // 304
	})

	app.Get("/login", func(c *fiber.Ctx) error {
		return c.Redirect("/api/v1/auth/login", fiber.StatusTemporaryRedirect) // 307
	})

	app.Get("/legacy/download", func(c *fiber.Ctx) error {
		return c.Redirect("/api/v1/download", fiber.StatusPermanentRedirect) // 308
	})

	// --- 4xx Client Errors ---
	app.Post("/api/v1/checkout", func(c *fiber.Ctx) error {
		roll := rand.Float32()
		if roll < 0.50 {
			return c.JSON(fiber.Map{
				"status":  "success",
				"orderId": fmt.Sprintf("ord_%x", rand.Intn(1000000)),
			})
		} else if roll < 0.80 {
			return fiber.NewError(fiber.StatusBadRequest, "Invalid payment card token or CVV mismatch")
		} else {
			return fiber.NewError(fiber.StatusServiceUnavailable, "Payment Gateway timeout connecting to bank API")
		}
	})

	app.Get("/api/v1/auth/login", func(c *fiber.Ctx) error {
		if rand.Float32() < 0.70 {
			return c.SendString("JWT Token generated: eyJhbGciOi...")
		}
		return fiber.NewError(fiber.StatusUnauthorized, "Invalid username or password")
	})

	app.Get("/admin/metrics/secret", func(c *fiber.Ctx) error {
		return fiber.NewError(fiber.StatusForbidden, "Access denied: insufficient IAM permissions")
	})

	app.Get("/api/v1/products/unknown-sku", func(c *fiber.Ctx) error {
		return fiber.NewError(fiber.StatusNotFound, "Product SKU #88124 not found in catalog")
	})

	app.Post("/api/v1/users/register", func(c *fiber.Ctx) error {
		return fiber.NewError(fiber.StatusConflict, "Email address 'user@example.com' already exists")
	})

	app.Put("/api/v1/users/profile", func(c *fiber.Ctx) error {
		return fiber.NewError(fiber.StatusUnprocessableEntity, "Validation failed: age must be greater than 18")
	})

	app.Post("/api/v1/ai/generate", func(c *fiber.Ctx) error {
		return fiber.NewError(fiber.StatusTooManyRequests, "Rate limit exceeded (10 req/min). Retry in 45s")
	})

	// --- 5xx Server Errors ---
	app.Get("/api/v1/panic", func(c *fiber.Ctx) error {
		panic("runtime error: unexpected nil pointer in order_dispatcher.go:88")
	})

	app.Get("/api/v1/upstream/grpc", func(c *fiber.Ctx) error {
		return fiber.NewError(fiber.StatusBadGateway, "Upstream gRPC payment-service unreachable (connection refused)")
	})

	app.Get("/api/v1/database/heavy-export", func(c *fiber.Ctx) error {
		return fiber.NewError(fiber.StatusGatewayTimeout, "SQL execution timeout after 30000ms")
	})

	addr := fmt.Sprintf("127.0.0.1:%d", *portFlag)

	// In banner ASCII art khi ở chế độ stdout/plain
	if !isJSON {
		pid := os.Getpid()
		fmt.Printf("\n \x1b[36m┌───────────────────────────────────────────────────┐\x1b[0m \n")
		fmt.Printf(" \x1b[36m│       UwULog Realistic Fiber Simulator v1.0       │\x1b[0m \n")
		fmt.Printf(" \x1b[36m│                  Fiber v2.52.15                   │\x1b[0m \n")
		fmt.Printf(" \x1b[36m│               \x1b[0mhttp://%s\x1b[36m               │\x1b[0m \n", addr)
		fmt.Printf(" \x1b[36m│                                                   │\x1b[0m \n")
		fmt.Printf(" \x1b[36m│\x1b[0m Handlers ............ \x1b[36m24\x1b[0m  Processes ........... \x1b[36m1\x1b[0m \x1b[36m│\x1b[0m \n")
		fmt.Printf(" \x1b[36m│\x1b[0m Prefork ....... \x1b[36mDisabled\x1b[0m  PID ............. \x1b[36m%-5d\x1b[0m \x1b[36m│\x1b[0m \n", pid)
		fmt.Printf(" \x1b[36m└───────────────────────────────────────────────────┘\x1b[0m \n\n")
	}

	// 5. Chạy Fiber Server ở background goroutine
	go func() {
		if err := app.Listen(addr); err != nil && !errors.Is(err, http.ErrServerClosed) {
			fmt.Fprintf(os.Stderr, "Fiber server error: %v\n", err)
		}
	}()

	// Đợi Fiber Server khởi động và bind port
	time.Sleep(250 * time.Millisecond)

	// 6. Worker tự động gửi request liên tục theo tốc độ cấu hình
	routes := []struct {
		method string
		path   string
	}{
		// 2xx Success
		{"GET", "/"},
		{"GET", "/healthz"},
		{"GET", "/api/v1/users"},
		{"POST", "/api/v1/users"},
		{"PUT", "/api/v1/settings"},
		{"PATCH", "/api/v1/users/status"},
		{"DELETE", "/api/v1/cache/flush"},
		{"HEAD", "/api/v1/ping"},
		{"OPTIONS", "/api/v1/cors"},
		{"GET", "/api/v1/query"},

		// 3xx Redirection & Cache
		{"GET", "/v1/old-endpoint"},
		{"GET", "/docs"},
		{"GET", "/static/app.bundle.js"},
		{"GET", "/login"},
		{"GET", "/legacy/download"},

		// 4xx Client Errors
		{"POST", "/api/v1/checkout"},
		{"GET", "/api/v1/auth/login"},
		{"GET", "/admin/metrics/secret"},
		{"GET", "/api/v1/products/unknown-sku"},
		{"GET", "/api/v1/not-found"},
		{"POST", "/api/v1/users/register"},
		{"PUT", "/api/v1/users/profile"},
		{"POST", "/api/v1/ai/generate"},

		// 5xx Server Errors
		{"GET", "/api/v1/panic"},
		{"GET", "/api/v1/upstream/grpc"},
		{"GET", "/api/v1/database/heavy-export"},
	}

	ips := []string{
		"127.0.0.1",
		"192.168.1.105",
		"10.0.4.12",
		"172.16.2.88",
		"8.8.8.8",
		"1.1.1.1",
		"142.250.190.46",
		"31.13.71.36",
	}

	userAgents := []string{
		"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
		"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15",
		"PostmanRuntime/7.32.3",
		"curl/8.4.0",
		"Googlebot/2.1 (+http://www.google.com/bot.html)",
	}

	client := &http.Client{
		Timeout: 2 * time.Second,
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return http.ErrUseLastResponse // Không tự động follow redirect để server in log trực tiếp mã 3xx
		},
	}

	var ticker *time.Ticker
	if *rateFlag > 0 {
		interval := time.Second / time.Duration(*rateFlag)
		ticker = time.NewTicker(interval)
		defer ticker.Stop()
	}

	baseURL := fmt.Sprintf("http://%s", addr)

	for i := 0; i < *countFlag; i++ {
		if ticker != nil {
			<-ticker.C
		}

		target := routes[rand.Intn(len(routes))]
		reqURL := fmt.Sprintf("%s%s", baseURL, target.path)

		req, err := http.NewRequest(target.method, reqURL, nil)
		if err != nil {
			continue
		}

		// Gán IP giả lập, User-Agent và Request-ID ngẫu nhiên
		req.Header.Set("X-Forwarded-For", ips[rand.Intn(len(ips))])
		req.Header.Set("User-Agent", userAgents[rand.Intn(len(userAgents))])
		req.Header.Set("X-Request-ID", fmt.Sprintf("tr-%08x", rand.Uint32()))

		// Gửi request tới Fiber Server (Server sẽ tự in log ra stdout theo cấu hình)
		resp, err := client.Do(req)
		if err == nil && resp != nil {
			_ = resp.Body.Close()
		}
	}
}
