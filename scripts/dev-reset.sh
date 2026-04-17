#!/bin/bash
# Quick dev environment reset: rebuild, reset DB, reseed, restart services
# Usage: ./scripts/dev-reset.sh

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

echo "🔄 Dev Environment Reset"
echo "========================"

# Load environment
source .env

# Stop services
echo "⏹️  Stopping services..."
pkill -f "cargo run|npm run dev|vite dev" 2>/dev/null || true
sleep 2

# Reset database
echo "🗑️  Resetting database..."
docker compose down -v
sleep 2
docker compose up -d db
sleep 5

# Build backend (release)
echo "🔨 Building backend..."
CARGO_TARGET_DIR=/scratch/homorg-target cargo build --release 2>&1 | grep -E "Finished|Compiling homorg" || true

# Start backend
echo "🚀 Starting backend..."
DATABASE_URL="postgres://homorg:homorg_dev@localhost:5432/homorg" \
CARGO_TARGET_DIR=/scratch/homorg-target \
/scratch/homorg-target/release/homorg &> /tmp/backend.log &
sleep 5
grep -i "listening" /tmp/backend.log | head -1

# Start frontend
echo "🌐 Starting frontend..."
cd web
npm run dev -- --host &> /tmp/frontend.log &
sleep 5
grep -i "local:" /tmp/frontend.log | head -1
cd ..

# Create admin user
echo "👤 Creating admin user..."
curl -s -X POST http://localhost:8080/api/auth/setup \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "seedpassword123"
  }' > /dev/null 2>&1 && echo "   ✅ admin / seedpassword123" || echo "   ⚠️  Setup already done"

echo ""
echo "✅ Ready for development!"
echo "   Backend:  http://localhost:8080"
echo "   Frontend: https://localhost:5173 (or 5174/5175)"
echo "   Admin:    admin / seedpassword123"
