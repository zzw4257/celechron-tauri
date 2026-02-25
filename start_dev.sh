#!/bin/bash

echo "======================================"
echo "🚀 Celechron Robust Development Start"
echo "======================================"

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "[1/3] Killing ONLY processes on port 1420 (Vite dev server)..."
# Only kill what's on port 1420 — nothing else
lsof -ti:1420 | xargs kill -9 2>/dev/null
lsof -ti:1421 | xargs kill -9 2>/dev/null

echo "[2/3] Clearing Vite physical cache..."
rm -rf "$PROJECT_DIR/node_modules/.vite"

echo "[3/3] Starting Tauri Dev Server..."
cd "$PROJECT_DIR" && npm run tauri dev
