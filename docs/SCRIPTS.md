# Development Scripts

This directory contains scripts to help with development and CI debugging.

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