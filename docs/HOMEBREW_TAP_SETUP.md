# Homebrew Tap Setup Guide

This guide explains how to set up and maintain the Homebrew tap for Cascade CLI.

## Initial Setup

### 1. Create the Tap Repository

**Option A: Using GitHub Web Interface**
1. Go to https://github.com/new
2. Repository name: `homebrew-cascade-cli` (MUST be this exact name)
3. Description: "Homebrew tap for Cascade CLI"
4. Make it Public
5. Do NOT initialize with README (the script will create one)
6. Click "Create repository"

**Option B: Using GitHub CLI**
```bash
gh repo create JAManfredi/homebrew-cascade-cli --public --description "Homebrew tap for Cascade CLI"
```

### 2. Run the Setup Script

Once the repository exists, run:

```bash
./scripts/setup-homebrew-tap.sh
```

This script will:
- Clone the tap repository
- Create the proper `Formula/` directory structure
- Copy the formula file
- Create a README
- Push everything to GitHub

## Keeping the Tap Updated

### Automatic Updates (Recommended)

The GitHub Actions workflow (`.github/workflows/update-homebrew-tap.yml`) automatically updates the tap when:
- A new release is published
- You manually trigger it with a version

**Manual trigger:**
1. Go to Actions tab in the cascade-cli repo
2. Select "Update Homebrew Tap"
3. Click "Run workflow"
4. Enter version (e.g., `v0.1.2`)

### Manual Updates

To manually sync the formula:

```bash
./scripts/sync-homebrew-formula.sh
```

This copies the current formula from the main repo to the tap.

### Updating for New Releases

When releasing a new version:

1. **Update the formula in the main repo:**
   ```bash
   # Edit homebrew/cascade-cli.rb
   # Update version number
   # Update download URLs to new version
   # Update SHA256 checksums
   ```

2. **Calculate new checksums:**
   ```bash
   # Download the release assets
   curl -L -o cc-macos-arm64.tar.gz https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.2/cc-macos-arm64.tar.gz
   curl -L -o cc-macos-x64.tar.gz https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.2/cc-macos-x64.tar.gz
   
   # Get checksums
   shasum -a 256 cc-macos-arm64.tar.gz
   shasum -a 256 cc-macos-x64.tar.gz
   ```

3. **Sync to tap:**
   - Either wait for GitHub Actions (if set up)
   - Or run `./scripts/sync-homebrew-formula.sh`

## How It Works

```
cascade-cli repo                    homebrew-cascade-cli repo
├── homebrew/                       ├── Formula/
│   └── cascade-cli.rb   ──sync──>  │   └── cascade-cli.rb
```

The tap repository only contains the formula file in the `Formula/` directory. Homebrew knows to look there when users run `brew tap JAManfredi/cascade-cli`.

## User Installation

Once the tap is set up, users can install with:

```bash
# One-time tap setup
brew tap JAManfredi/cascade-cli

# Install
brew install cascade-cli

# Future updates
brew update
brew upgrade cascade-cli
```

## Troubleshooting

### "Repository not found" error
- Make sure the repository is named exactly `homebrew-cascade-cli`
- Ensure it's public
- Check you have push access

### Formula not found after tapping
- Ensure the formula is in the `Formula/` directory
- File must be named `cascade-cli.rb`
- Try `brew tap --repair`

### Checksum mismatch
- Download the actual release assets
- Recalculate SHA256: `shasum -a 256 filename.tar.gz`
- Update both ARM64 and x64 checksums

## Benefits of Using a Tap

1. **Clean installation**: `brew install cascade-cli` instead of downloading formula
2. **Auto-updates**: Users get updates with `brew update`
3. **Version management**: Homebrew handles upgrades and rollbacks
4. **Discovery**: Shows up in `brew search cascade`
5. **Professional**: Standard way to distribute Homebrew formulas