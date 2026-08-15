#!/usr/bin/env python3
"""
Python JSON Log Generator for uwulog-rust
Generates realistic Python-style JSON logs with standard Python logging levels,
module names, execution metadata, and realistic exception tracebacks.
"""

import argparse
import datetime
import json
import random
import sys
import time
import uuid


# Standard Python Logging Levels
LEVELS = [
    ("DEBUG", 0.20),
    ("INFO", 0.50),
    ("WARNING", 0.15),
    ("ERROR", 0.12),
    ("CRITICAL", 0.03),
]

LOGGERS = [
    ("app.api.v1.auth", "auth.py", "authenticate_user", 84),
    ("app.api.v1.payments", "payments.py", "process_checkout", 129),
    ("app.services.image", "image_service.py", "resize_avatar", 45),
    ("app.database.pool", "db_pool.py", "acquire_connection", 210),
    ("uvicorn.access", "access.py", "log_access", 38),
    ("celery.worker", "tasks.py", "execute_background_job", 73),
    ("sqlalchemy.engine.Engine", "base.py", "execute", 1198),
    ("redis.connection", "connection.py", "send_command", 182),
    ("app.core.security", "jwt.py", "verify_token", 56),
    ("urllib3.connectionpool", "connectionpool.py", "_make_request", 442),
]

HTTP_METHODS = ["GET", "POST", "PUT", "DELETE", "PATCH"]
PATHS = [
    "/api/v1/login",
    "/api/v1/users/profile",
    "/api/v1/payments/checkout",
    "/api/v1/images/upload",
    "/healthz",
    "/metrics",
    "/api/v2/products/search",
    "/api/v1/orders/1029",
    "/api/v1/logout",
]
STATUS_CODES = [200, 201, 204, 400, 401, 403, 404, 422, 500, 502, 503]
IPS = [
    "192.168.1.15",
    "10.0.4.22",
    "172.20.0.10",
    "127.0.0.1",
    "8.8.8.8",
    "1.1.1.1",
    "142.250.190.46",
    "104.244.42.1",
]

INFO_MESSAGES = [
    "User logged in successfully",
    "Request processed successfully in {latency}ms",
    "Database connection pool initialized (min=5, max=20)",
    "Image processed and uploaded to S3: '{image}'",
    "Background celery task '{task_id}' dispatched",
    "Token verified for user_id={user_id}",
    "HTTP Request: {method} {path} - Status {status}",
    "Cache hit for key 'session:{user_id}'",
    "Health check passed: all downstream services OK",
]

WARNING_MESSAGES = [
    "High memory usage detected on worker: 87%",
    "Slow database query ({latency}ms): SELECT * FROM orders WHERE status = 'pending'",
    "Rate limit threshold reached for client IP: {ip}",
    "Cache miss for key 'product_catalog:featured'",
    "Deprecated API endpoint accessed: {path}",
    "Connection pool nearly exhausted: 18/20 active connections",
    "Retry attempt 2/5 for external service webhook",
]

ERROR_MESSAGES = [
    "Database connection failed: Connection refused to postgres:5432",
    "Failed to decode JSON payload in request body",
    "Payment gateway timeout after 5000ms",
    "Unhandled exception while processing request {path}",
    "Failed to authenticate user: invalid signature",
    "PermissionDenied: User {user_id} lacks 'admin:write' role",
    "FileNotFoundError: Avatar asset missing on disk: '{image}'",
]

CRITICAL_MESSAGES = [
    "Critical: Database replica lag exceeded 300s",
    "Service unavailable: Redis cluster leader election in progress",
    "Out of memory error: Celery worker killed by OS killer",
    "Emergency: SSL certificate expires in less than 24 hours",
    "Unhandled SystemExit: Main worker thread terminated unexpectedly",
]

PYTHON_EXCEPTIONS = [
    """Traceback (most recent call last):
  File "app/api/v1/auth.py", line 95, in authenticate_user
    payload = jwt.decode(token, SECRET_KEY, algorithms=["HS256"])
  File "jose/jwt.py", line 124, in decode
    raise ExpiredSignatureError("Signature has expired.")
jose.exceptions.ExpiredSignatureError: Signature has expired.""",
    """Traceback (most recent call last):
  File "app/database/pool.py", line 215, in acquire_connection
    conn = await self._pool.acquire(timeout=5.0)
  File "asyncpg/pool.py", line 612, in acquire
    raise asyncio.TimeoutError("Could not acquire connection within timeout")
TimeoutError: Could not acquire connection within timeout""",
    """Traceback (most recent call last):
  File "app/services/image.py", line 52, in resize_avatar
    with Image.open(file_path) as img:
  File "PIL/Image.py", line 3247, in open
    raise FileNotFoundError(f"No such file or directory: '{file_path}'")
FileNotFoundError: [Errno 2] No such file or directory: 'uploads/avatars/user_99.png'""",
    """Traceback (most recent call last):
  File "app/api/v1/payments.py", line 140, in process_checkout
    charge = stripe.Charge.create(amount=amount, currency="usd", source=token)
  File "stripe/api_resources/charge.py", line 45, in create
    raise stripe.error.CardError("Your card was declined.", param="card", code="card_declined")
stripe.error.CardError: Request req_981a2: Your card was declined.""",
]


def pick_level():
    r = random.random()
    cumulative = 0.0
    for level, weight in LEVELS:
        cumulative += weight
        if r <= cumulative:
            return level
    return "INFO"


def generate_log_entry(idx: int, is_live: bool) -> dict:
    level = pick_level()
    logger_name, filename, func_name, lineno = random.choice(LOGGERS)

    if is_live:
        ts = datetime.datetime.now(datetime.timezone.utc)
    else:
        # Lùi thời gian ngẫu nhiên trong 24 giờ qua
        minutes_ago = random.randint(0, 1440)
        seconds_ago = random.randint(0, 59)
        ts = datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(
            minutes=minutes_ago, seconds=seconds_ago
        )

    user_id = f"user_{random.randint(100, 999)}"
    ip = random.choice(IPS)
    method = random.choice(HTTP_METHODS)
    path = random.choice(PATHS)
    status = random.choice(STATUS_CODES)
    latency = random.randint(5, 1850)
    image = f"avatar_{random.randint(1, 50)}.jpg"
    task_id = str(uuid.uuid4())[:8]

    if level == "DEBUG":
        message = f"Executing query: SELECT id, name FROM users WHERE id = {random.randint(1, 500)} LIMIT 1"
    elif level == "INFO":
        tmpl = random.choice(INFO_MESSAGES)
        message = tmpl.format(
            latency=latency,
            image=image,
            task_id=task_id,
            user_id=user_id,
            method=method,
            path=path,
            status=status,
        )
    elif level == "WARNING":
        tmpl = random.choice(WARNING_MESSAGES)
        message = tmpl.format(
            latency=latency,
            ip=ip,
            path=path,
        )
    elif level == "ERROR":
        tmpl = random.choice(ERROR_MESSAGES)
        message = tmpl.format(
            path=path,
            user_id=user_id,
            image=image,
        )
    else:  # CRITICAL
        message = random.choice(CRITICAL_MESSAGES)

    entry = {
        "timestamp": ts.isoformat(),
        "level": level,
        "levelname": level,
        "name": logger_name,
        "logger": logger_name,
        "source": logger_name,
        "message": message,
        "filename": filename,
        "funcName": func_name,
        "lineno": lineno,
        "process": random.randint(1000, 9999),
        "thread_name": random.choice(["MainThread", "ThreadPoolExecutor-0_1", "uvicorn-worker"]),
        "trace_id": f"tr-{uuid.uuid4().hex[:12]}",
        "span_id": f"sp-{uuid.uuid4().hex[:8]}",
    }

    # Bổ sung trường HTTP & User metadata ngẫu nhiên
    if random.random() < 0.65:
        entry["user_id"] = user_id
    if random.random() < 0.55:
        entry["ip"] = ip
        entry["method"] = method
        entry["path"] = path
        entry["status_code"] = status
        entry["latency_ms"] = latency

    # Đối với ERROR và CRITICAL: thêm traceback chuẩn Python
    if level in ("ERROR", "CRITICAL") and random.random() < 0.70:
        entry["exc_info"] = random.choice(PYTHON_EXCEPTIONS)

    return entry


def main():
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", line_buffering=True)
    if hasattr(sys.stderr, "reconfigure"):
        sys.stderr.reconfigure(encoding="utf-8", line_buffering=True)

    parser = argparse.ArgumentParser(
        description="Generate realistic Python JSON logs for uwulog-rust"
    )
    parser.add_argument(
        "-n",
        "--count",
        type=int,
        default=500000,
        help="Total number of logs to generate (default: 500,000)",
    )
    parser.add_argument(
        "-r",
        "--rate",
        type=int,
        default=0,
        help="Logs per second rate limit (0 for unlimited speed, e.g. 30 for streaming)",
    )
    args = parser.parse_args()

    is_live = args.rate > 0
    delay = 1.0 / args.rate if args.rate > 0 else 0

    try:
        for i in range(args.count):
            entry = generate_log_entry(i, is_live)
            print(json.dumps(entry, ensure_ascii=False), flush=True)

            if delay > 0:
                time.sleep(delay)
    except (KeyboardInterrupt, BrokenPipeError):
        pass


if __name__ == "__main__":
    main()
