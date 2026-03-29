#!/bin/bash
# Build script for Conflux Server Dashboard
# This script runs from the dashboard directory to build the frontend

set -e

echo "Building Conflux Server Dashboard..."

cd "$(dirname "$0")"

# Install dependencies if needed (or if package.json is newer than node_modules)
if [ ! -d "node_modules" ] || [ "package.json" -nt "node_modules/.package-lock.json" ]; then
    echo "Installing dependencies..."
    npm install
fi

# Build the dashboard
echo "Building Vite project..."
npm run build

echo "Dashboard build complete. Output: dist/"
