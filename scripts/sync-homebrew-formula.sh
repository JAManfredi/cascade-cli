#!/bin/bash
# Script to manually sync the Homebrew formula to the tap repository

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo "🍺 Syncing Homebrew formula to tap repository..."

# Check if gh is installed
if ! command -v gh &> /dev/null; then
    echo -e "${RED}Error: GitHub CLI (gh) is required but not installed.${NC}"
    echo "Install it with: brew install gh"
    exit 1
fi

# Check if tap repo exists
if ! gh repo view JAManfredi/homebrew-cascade-cli &> /dev/null; then
    echo -e "${RED}Error: Tap repository JAManfredi/homebrew-cascade-cli not found.${NC}"
    echo "Run ./scripts/setup-homebrew-tap.sh first to create it."
    exit 1
fi

# Get the directory of this script
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

# Create temp directory
TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

# Clone the tap repo
echo "📥 Cloning tap repository..."
git clone https://github.com/JAManfredi/homebrew-cascade-cli.git
cd homebrew-cascade-cli

# Ensure Formula directory exists
mkdir -p Formula

# Copy the formula
echo "📋 Copying formula..."
cp "$PROJECT_ROOT/homebrew/cascade-cli.rb" Formula/cascade-cli.rb

# Check if there are changes
if git diff --quiet; then
    echo -e "${YELLOW}No changes to sync.${NC}"
    cd "$PROJECT_ROOT"
    rm -rf "$TEMP_DIR"
    exit 0
fi

# Commit and push
echo "📤 Pushing changes..."
git add Formula/cascade-cli.rb
git commit -m "Sync formula from main repository

Synced from: https://github.com/JAManfredi/cascade-cli
Date: $(date -u +"%Y-%m-%d %H:%M:%S UTC")"
git push

echo -e "${GREEN}✅ Formula synced successfully!${NC}"

# Cleanup
cd "$PROJECT_ROOT"
rm -rf "$TEMP_DIR"

echo ""
echo "Users can now install with:"
echo "  brew tap JAManfredi/cascade-cli"
echo "  brew install cascade-cli"