# Development Scripts

This directory contains scripts to help with development, release management, and CI debugging.

## 🚀 Release Management Scripts

### `bump-version.sh`
**Automated version bump script** that handles all version references across the project.

```bash
./scripts/bump-version.sh <new-version>
```

**Features:**
- Updates `Cargo.toml` package version
- Updates Homebrew formula URLs and version fields 
- Refreshes `Cargo.lock` dependencies
- Searches for other version references in docs/code
- Creates git commit with standardized message
- Creates annotated git tag with release notes
- Cross-platform compatible (macOS/Linux)
- Includes validation and safety checks
- Requires user confirmation before proceeding

**Usage Examples:**
```bash
# Bump to next patch version
./scripts/bump-version.sh 0.1.7

# Bump to next minor version  
./scripts/bump-version.sh 0.2.0

# Bump to next major version
./scripts/bump-version.sh 1.0.0
```

**What it does:**
1. Validates version format (X.Y.Z)
2. Checks for clean git working directory
3. Shows current vs new version and asks for confirmation
4. Updates all version references automatically
5. Creates standardized commit message and git tag
6. Provides clear next steps for release process

**Next steps after running:**
1. Review changes: `git show HEAD`
2. Push commits: `git push origin master`
3. Push tag: `git push origin vX.Y.Z`
4. Create GitHub release from the tag
5. Build and upload release binaries

## 🔧 CI Simulation Scripts

### `pre-push-check.sh`
**The main script to run before pushing to GitHub.**

```bash
./scripts/pre-push-check.sh
```

Runs all the same checks as GitHub Actions CI:
- Code formatting (`cargo fmt --check`)
- Linting (`cargo clippy`)
- Build verification
- Unit tests
- Integration tests 
- CLI binary test
- Documentation checks
- Security audit (if available)

### `ci-simulation.sh`
**Enhanced CI environment simulation** that closely replicates GitHub Actions.

```bash
./scripts/ci-simulation.sh
```

Features:
- Sets CI environment variables (`CI=true`, `GITHUB_ACTIONS=true`)
- Configures Git like CI does
- Runs tests with CI-specific timeouts and concurrency limits
- Checks test isolation (runs tests twice)
- Monitors resource cleanup
- Simulates MSRV (Minimum Supported Rust Version) checks

This catches more environment-specific issues than the basic pre-push check.

### `docker-ci-simulation.sh`
**Docker-based exact CI environment replica** for catching platform-specific issues.

```bash
./scripts/docker-ci-simulation.sh [OPTIONS]
```

Options:
- `--rust-version VERSION` - Rust version to use (default: stable)
- `--os VERSION` - OS version (ubuntu-latest, ubuntu-20.04, etc.)
- `--keep-container` - Keep container after test for debugging
- `--interactive` - Drop into interactive shell after tests
- `--help` - Show help

Examples:
```bash
# Basic simulation
./scripts/docker-ci-simulation.sh

# Debug mode - keeps container and starts interactive session
./scripts/docker-ci-simulation.sh --interactive --keep-container

# Test with specific Rust version
./scripts/docker-ci-simulation.sh --rust-version 1.82.0

# Test with Ubuntu 20.04 (like older CI environments)
./scripts/docker-ci-simulation.sh --os ubuntu-20.04
```

This creates an **exact replica** of the GitHub Actions Ubuntu environment.

## 🐛 Debugging Scripts

### `debug-integration-tests.sh`
**Detailed integration test failure analysis.**

```bash
./scripts/debug-integration-tests.sh
```

Features:
- Runs each integration test module individually
- Provides specific debugging suggestions for each module
- Shows detailed environment information
- Identifies common failure patterns
- Suggests targeted fixes for specific test types

Use this when integration tests are failing to understand **why** they're failing.

## 📋 Usage Workflow

### Daily Development
```bash
# Before every push
./scripts/pre-push-check.sh
```

### Creating a Release
```bash
# 1. Bump version (creates commit and tag)
./scripts/bump-version.sh 0.1.7

# 2. Run final validation
./scripts/pre-push-check.sh

# 3. Push everything
git push origin master
git push origin v0.1.7

# 4. Create GitHub release and upload binaries
```

### When CI Fails But Local Tests Pass
```bash
# Step 1: Try enhanced simulation
./scripts/ci-simulation.sh

# Step 2: If still passing, debug integration tests specifically
./scripts/debug-integration-tests.sh

# Step 3: If nothing found, use Docker for exact CI replica
./scripts/docker-ci-simulation.sh --interactive
```

### When Integration Tests Fail
```bash
# Debug specific failures
./scripts/debug-integration-tests.sh

# Test in CI-like environment
./scripts/ci-simulation.sh

# If issue persists, isolate in Docker
./scripts/docker-ci-simulation.sh --keep-container
```

### When You Need Exact CI Environment
```bash
# Full Docker CI simulation
./scripts/docker-ci-simulation.sh

# Interactive debugging in CI environment
./scripts/docker-ci-simulation.sh --interactive

# Test with specific configurations
./scripts/docker-ci-simulation.sh --rust-version 1.82.0 --os ubuntu-20.04
```

## 🎯 Common CI Issues These Scripts Catch

### Environment-Specific Issues
- **Default Git branch differences** (master vs main)
- **File permission issues** (Unix vs Windows executable bits)
- **Line ending differences** (CRLF vs LF)
- **Path separator issues** (/ vs \)

### Resource and Timing Issues
- **Test isolation failures** (state pollution between tests)
- **Resource cleanup problems** (leftover temp files)
- **Timeout issues** (tests that hang in CI)
- **Concurrency problems** (race conditions in parallel tests)

### Configuration Issues
- **Missing dependencies** (tools not installed in CI)
- **Environment variable differences**
- **Git configuration mismatches**
- **Rust version compatibility** (MSRV violations)

## 🔧 Script Dependencies

### System Requirements
- **Bash** (all scripts)
- **Docker** (for `docker-ci-simulation.sh`)
- **Rust/Cargo** (obviously)
- **Git** (for repo operations)

### Optional Tools
- `cargo-audit` - For security audits (install: `cargo install cargo-audit`)
- `timeout` command - For test timeouts (usually available on Unix systems)

## 💡 Tips

1. **Run `pre-push-check.sh` before every push** - it's fast and catches most issues
2. **Use `ci-simulation.sh` when you suspect environment issues** - it's more thorough
3. **Use `docker-ci-simulation.sh` for stubborn CI failures** - it's the most accurate
4. **Keep containers for debugging** with `--keep-container` flag
5. **Test with multiple Rust versions** if you suspect MSRV issues

## 🚨 Troubleshooting

### Docker Issues
```bash
# Check Docker is running
docker info

# Clean up old containers
docker system prune -f
```

### Permission Issues
```bash
# Make scripts executable
chmod +x scripts/*.sh
```

### Path Issues
```bash
# Run from project root
cd /path/to/cascade-cli
./scripts/pre-push-check.sh
```

## 📚 Related Documentation

- [DEVELOPMENT.md](../docs/DEVELOPMENT.md) - Full development guide
- [CI_COMPATIBILITY.md](../docs/CI_COMPATIBILITY.md) - CI troubleshooting guide
- [.github/workflows/ci.yml](../.github/workflows/ci.yml) - Actual CI configuration 