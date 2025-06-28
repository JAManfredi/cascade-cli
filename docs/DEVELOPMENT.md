# Development Guide

This guide covers development workflows, testing, and contribution guidelines for Cascade CLI.

## Table of Contents

- [Quick Start](#quick-start)
- [Pre-Push Validation](#pre-push-validation)
- [Testing](#testing)
- [Code Quality](#code-quality)
- [Release Process](#release-process)
- [Contributing](#contributing)

## Quick Start

### Prerequisites

- Rust 1.82.0 or later
- Git
- macOS, Linux, or Windows

### Setup

```bash
# Clone the repository
git clone https://github.com/jared/cascade-cli.git
cd cascade-cli

# Build the project
cargo build

# Run tests
cargo test

# Install for local development
cargo install --path .
```

## Pre-Push Validation

**Always run local validation before pushing to GitHub!** This catches all issues locally that would otherwise fail in CI.

### Automated Script

Use our comprehensive pre-push validation script:

```bash
./scripts/pre-push-check.sh
```

This script runs **all the same checks as GitHub CI**:

- ✅ Code formatting (`cargo fmt --check`)
- ✅ Clippy linting (`cargo clippy -- -D warnings`)
- ✅ Unit tests (`cargo test`)
- ✅ Integration tests (if present)
- ✅ Binary compilation (`cargo build --release`)
- ✅ Documentation generation (`cargo doc`)
- ✅ Documentation tests (`cargo test --doc`)
- ⚠️ Security audit (`cargo audit` - optional)

### Manual Commands

If you prefer to run checks individually:

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Run clippy (with auto-fixes)
cargo clippy --fix --all-targets --all-features

# Run clippy (check only)
cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
cargo test

# Build release binary
cargo build --release

# Generate docs
cargo doc

# Test docs
cargo test --doc

# Security audit (optional)
cargo install cargo-audit  # First time only
cargo audit
```

## Testing

### Unit Tests

```bash
# Run all unit tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run tests in specific module
cargo test stack::tests
```

### Integration Tests

```bash
# Run integration tests (when available)
cargo test --test integration
```

### Test Coverage

```bash
# Install coverage tool
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out html
open tarpaulin-report.html
```

## Code Quality

### Formatting

We use standard Rust formatting:

```bash
cargo fmt
```

### Linting

We enforce strict linting with Clippy:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Common fixes:
- Use `_variable` for unused variables
- Add `#[allow(clippy::too_many_arguments)]` for functions with many parameters
- Use iterator methods instead of manual loops
- Remove redundant clones and allocations

### Documentation

All public APIs must be documented:

```bash
# Generate docs
cargo doc --open

# Test documentation examples
cargo test --doc
```

## Release Process

### Version Management

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md` with new version
3. Commit changes: `git commit -m "Release v1.x.x"`
4. Create tag: `git tag v1.x.x`
5. Push tag: `git push origin v1.x.x`

### Automated Release

The release is automated via GitHub Actions when a tag is pushed:

1. **Build**: Compiles for multiple platforms
2. **Test**: Runs full test suite
3. **Package**: Creates distribution archives
4. **Publish**: 
   - Creates GitHub release with binaries
   - Publishes to crates.io
   - Updates Homebrew formula

### Manual Release Steps

If manual release is needed:

```bash
# Build for release
cargo build --release

# Create distribution archive
tar -czf cascade-cli-v1.x.x-macos.tar.gz -C target/release cascade-cli

# Calculate SHA256 for Homebrew
shasum -a 256 cascade-cli-v1.x.x-macos.tar.gz

# Publish to crates.io
cargo publish
```

## Contributing

### Code Style

- Follow Rust naming conventions
- Use meaningful variable and function names
- Add comments for complex logic
- Keep functions focused and small
- Use `Result<T>` for error handling

### Commit Messages

Use conventional commit format:

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: Feature
- `fix`: Bug fix  
- `docs`: Documentation changes
- `style`: Code style changes
- `refactor`: Code refactoring
- `test`: Test changes
- `chore`: Build/tool changes

### Pull Request Process

1. **Create branch**: `git checkout -b feature/your-feature`
2. **Make changes**: Follow code style guidelines
3. **Test locally**: Run `./scripts/pre-push-check.sh`
4. **Commit**: Use conventional commit messages
5. **Push**: `git push origin feature/your-feature`
6. **Open PR**: Include description and test results
7. **Review**: Address feedback and update
8. **Merge**: Squash and merge when approved

### Issue Reporting

When reporting bugs:

1. **Search existing issues** first
2. **Use issue template** if provided
3. **Include**:
   - OS and version
   - Rust version (`rustc --version`)
   - Cascade CLI version
   - Steps to reproduce
   - Expected vs actual behavior
   - Error messages/logs

### Feature Requests

When requesting features:

1. **Check roadmap** and existing issues
2. **Describe use case** and problem being solved
3. **Propose solution** if you have ideas
4. **Consider backwards compatibility**

## Development Tips

### Debugging

```bash
# Run with debug output
RUST_LOG=debug cargo run -- your-command

# Enable backtraces
RUST_BACKTRACE=1 cargo run -- your-command

# Use specific log levels
RUST_LOG=cascade_cli::stack=trace cargo run -- stack list
```

### Performance Profiling

```bash
# Install profiling tools
cargo install cargo-profdata

# Profile a command
cargo run --release -- your-command
```

### Cross-Platform Testing

Test on multiple platforms when possible:

- **macOS**: Primary development platform
- **Linux**: Ubuntu/Debian testing
- **Windows**: PowerShell and Command Prompt

### IDE Setup

Recommended VS Code extensions:

- `rust-analyzer`: Language server
- `CodeLLDB`: Debugging
- `crates`: Dependency management
- `Better TOML`: Cargo.toml editing

## Troubleshooting

### Common Issues

**Clippy errors after dependency updates:**
```bash
cargo clean
cargo clippy --fix --all-targets --all-features
```

**Test failures in CI but not locally:**
```bash
# Run in same environment as CI
cargo test --release
```

**Documentation generation fails:**
```bash
cargo clean
cargo doc --no-deps
```

**Build errors with new Rust version:**
```bash
# Update toolchain
rustup update stable
cargo update
```

### Getting Help

- **Documentation**: Check `/docs` folder
- **Issues**: Search GitHub issues
- **Discussions**: GitHub Discussions tab
- **Discord**: [Project Discord](link-if-available)

---

Happy coding! 🦀🌊 