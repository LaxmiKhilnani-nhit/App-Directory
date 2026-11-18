#!/usr/bin/env bash
# Diagnose "connection refused" to the App Directory server on the VM.
# Run from the repository root:  bash infra/scripts/check-server.sh
# Paste the entire output when asking for help.

set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

echo "=== public IP addresses on this VM ==="
ip -4 -br addr show 2>/dev/null || hostname -I

echo
echo "=== what is listening on 8090 ==="
if command -v ss >/dev/null; then
  ss -ltnp | grep ':8090' || echo "NOTHING IS LISTENING ON 8090"
else
  netstat -ltnp 2>/dev/null | grep ':8090' || echo "NOTHING IS LISTENING ON 8090"
fi

echo
echo "=== port conflict check ==="
if command -v lsof >/dev/null; then
  lsof -iTCP:8090 -sTCP:LISTEN || true
fi

echo
echo "=== containers ==="
docker compose -f infra/docker-compose.yml --env-file .env ps 2>&1 || true

echo
echo "=== api logs (last 40 lines) ==="
docker logs --tail 40 app-directory-api-1 2>&1 || true

echo
echo "=== health via loopback ==="
curl -sS -m 5 -o /tmp/appdir-health.json -w 'HTTP %{http_code}\n' http://127.0.0.1:8090/api/v1/healthz 2>&1 || true
cat /tmp/appdir-health.json 2>/dev/null || true

echo
echo "=== health via 172.168.50.50 (from this VM) ==="
curl -sS -m 5 -o /tmp/appdir-health-lan.json -w 'HTTP %{http_code}\n' http://172.168.50.50:8090/api/v1/healthz 2>&1 || true
cat /tmp/appdir-health-lan.json 2>/dev/null || true

echo
echo "=== firewall ==="
sudo ufw status 2>/dev/null || echo "ufw not available or not enabled"
