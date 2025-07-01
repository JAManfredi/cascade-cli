# Homebrew Tap for Cascade CLI

This is the Homebrew tap for Cascade CLI, containing the formula to install the `ca` command.

## Installation

```bash
# Add the tap
brew tap JAManfredi/cascade-cli

# Install Cascade CLI
brew install cascade-cli

# Verify installation
ca --version
```

## Updating the Formula

When releasing a new version of Cascade CLI:

### 1. Build Release Binaries

First, create GitHub release with binaries for both architectures:
- `ca-macos-arm64.tar.gz` (Apple Silicon)
- `ca-macos-x64.tar.gz` (Intel)

### 2. Generate SHA256 Checksums

Download the release binaries and generate checksums:

```bash
# Download releases
curl -L -O https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.6/ca-macos-arm64.tar.gz
curl -L -O https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.6/ca-macos-x64.tar.gz

# Generate checksums
shasum -a 256 ca-macos-arm64.tar.gz
shasum -a 256 ca-macos-x64.tar.gz
```

### 3. Update Formula

Edit `cascade-cli.rb`:

1. Update the `version` field
2. Update the URLs to point to the new release
3. Replace the SHA256 values with the checksums from step 2

### 4. Test the Formula

```bash
# Test installation
brew install --build-from-source cascade-cli

# Test the installed binary
ca --version
ca doctor

# Uninstall test version
brew uninstall cascade-cli
```

### 5. Commit and Push

```bash
git add cascade-cli.rb
git commit -m "Update Cascade CLI to v0.1.6"
git push origin main
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