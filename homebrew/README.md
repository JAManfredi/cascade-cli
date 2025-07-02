# Homebrew Integration

This directory contains information about Homebrew integration for Cascade CLI.

## Installation

```bash
# Add the tap
brew tap JAManfredi/cascade-cli

# Install Cascade CLI
brew install cascade-cli

# Verify installation
ca --version
```

## Automated Formula Updates

**The Homebrew formula is now automatically updated via GitHub Actions!** 🎉

### How It Works

1. **Release Creation**: When a new release is created (via `git tag vX.Y.Z && git push --tags`)
2. **Automatic Trigger**: The release workflow automatically triggers the Homebrew tap update
3. **Formula Update**: The tap repository gets a Pull Request with:
   - Updated version number
   - New download URLs
   - Fresh SHA256 checksums
   - Automated testing

### Repository Structure

- **Main Repository**: `JAManfredi/cascade-cli` (this repo)
  - Contains release workflows
  - Triggers tap updates automatically

- **Tap Repository**: `JAManfredi/homebrew-cascade-cli` 
  - Contains the actual formula file
  - Gets updated via automated PRs
  - Users install from this tap

### Manual Override (if needed)

If you need to manually trigger a formula update:

```bash
# From the main repository, trigger the workflow
gh workflow run update-homebrew-tap.yml -f version=v1.2.3
```

Or update the tap repository directly (not recommended):

```bash
# Clone the tap repository  
git clone https://github.com/JAManfredi/homebrew-cascade-cli.git
cd homebrew-cascade-cli

# Edit Formula/cascade-cli.rb manually
# Commit and push changes
```

## Formula Structure

The formula installs:
- The `ca` binary to the PATH
- Shell completions for bash, zsh, and fish
- Man pages (if available)

## Troubleshooting

### Architecture Detection

The formula automatically detects the system architecture and downloads the appropriate binary:
- Apple Silicon (M1/M2): `ca-macos-arm64.tar.gz`
- Intel: `ca-macos-x64.tar.gz`

### Installation Issues

If installation fails:

```bash
# Check formula syntax
brew audit cascade-cli

# Install with verbose output
brew install --verbose cascade-cli

# Check for conflicts
brew doctor
```

### Binary Issues

If the installed binary doesn't work:

```bash
# Check binary location
which ca

# Check binary permissions
ls -la $(which ca)

# Test directly
/usr/local/bin/ca --version
```

## Maintenance

This tap is maintained as part of the Cascade CLI project. For issues with the Homebrew formula, please file an issue in the main repository: https://github.com/JAManfredi/cascade-cli/issues