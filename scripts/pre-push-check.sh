#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_step() {
    echo -e "${BLUE}==>${NC} $1"
}

print_success() {
    echo -e "${GREEN}✅${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠️${NC} $1"
}

print_error() {
    echo -e "${RED}❌${NC} $1"
}

# Function to run a command and check its result
run_check() {
    local step_name="$1"
    local command="$2"
    
    print_step "$step_name"
    if eval "$command"; then
        print_success "$step_name passed"
        return 0
    else
        print_error "$step_name failed"
        return 1
    fi
}

# Function to run a command and exit immediately on failure (for quick checks)
run_check_fail_fast() {
    local step_name="$1"
    local command="$2"
    local fix_hint="${3:-}"
    
    print_step "$step_name"
    if eval "$command"; then
        print_success "$step_name passed"
        return 0
    else
        print_error "$step_name failed"
        if [ -n "$fix_hint" ]; then
            echo -e "${YELLOW}💡 Quick fix:${NC} $fix_hint"
        fi
        echo -e "${RED}❌ Stopping here to save time. Fix this issue and re-run.${NC}"
        exit 1
    fi
}

echo "🚀 Running all CI checks locally before push..."
echo "This will catch any issues before they reach GitHub!"
echo
print_warning "Note: Some environment-dependent issues (like default Git branch names)"
print_warning "may still occur in CI even if tests pass locally. Consider testing with"
print_warning "different Git configurations if CI failures don't match local results."
echo

# Configure git (in case it's not set)
git config --global user.name "$(git config user.name || echo 'Local User')" 2>/dev/null || true
git config --global user.email "$(git config user.email || echo 'user@local.dev')" 2>/dev/null || true

echo "🏃‍♂️ Running quick checks first (fail fast)..."
echo

# 1. Check formatting (fail fast)
run_check_fail_fast "Code Formatting" "cargo fmt --all -- --check" "cargo fmt"

# 2. Run Clippy (fail fast)
run_check_fail_fast "Clippy Linting" "cargo clippy --all-targets --all-features -- -D warnings" "cargo clippy --fix"

print_success "Quick checks passed! Proceeding with comprehensive testing..."
echo

echo "🧪 Running comprehensive tests and checks..."
echo

FAILED_CHECKS=()

# Platform-specific code reminder
print_step "Platform-specific Code Check"
if grep -r "#\[cfg(windows)\]" src/ >/dev/null 2>&1; then
    print_warning "Platform-specific code detected - ensure testing on CI"
    print_warning "Windows-specific code may behave differently in CI"
else
    print_success "No platform-specific code found"
fi

# Run Clippy with beta if available (matches CI matrix)
if rustup toolchain list | grep -q "beta"; then
    print_step "Clippy Beta (CI compatibility check)"
    if rustup run beta cargo clippy --all-targets --all-features -- -D warnings 2>/dev/null; then
        print_success "Clippy beta passed"
    else
        print_warning "Clippy beta failed - this may cause CI failures"
        print_warning "Consider running: rustup install beta && rustup run beta cargo clippy --fix"
    fi
else
    print_warning "Beta toolchain not installed (run: rustup install beta)"
fi

# Build check
if ! run_check "Build Check" "cargo build --verbose"; then
    FAILED_CHECKS+=("build")
fi

# Unit tests
if ! run_check "Unit Tests" "cargo test --lib --verbose"; then
    FAILED_CHECKS+=("unit-tests")
fi

# Integration tests
if ! run_check "Integration Tests" "cargo test --test '*' --verbose -- --test-threads=1"; then
    FAILED_CHECKS+=("integration-tests")
fi

# CLI binary test
if ! run_check "CLI Binary Test" "cargo build --release && ./target/release/csc --help > /dev/null"; then
    FAILED_CHECKS+=("cli-binary")
fi

# Documentation check
if ! run_check "Documentation Check" "cargo doc --no-deps --document-private-items"; then
    FAILED_CHECKS+=("docs")
fi

# Documentation tests
if ! run_check "Documentation Tests" "cargo test --doc"; then
    FAILED_CHECKS+=("doc-tests")
fi

# Security audit (optional - might not have cargo-audit installed)
print_step "Security Audit"
if command -v cargo-audit &> /dev/null; then
    if cargo audit; then
        print_success "Security audit passed"
    else
        print_warning "Security audit found issues"
        FAILED_CHECKS+=("security")
    fi
else
    print_warning "cargo-audit not installed (run: cargo install cargo-audit)"
fi

# Summary
echo
echo "🎯 Summary:"
if [ ${#FAILED_CHECKS[@]} -eq 0 ]; then
    print_success "All comprehensive checks passed! ✨ Safe to push to GitHub!"
    echo
    echo "📊 Completed checks:"
    echo "  ✅ Code formatting"
    echo "  ✅ Clippy linting" 
    echo "  ✅ Build"
    echo "  ✅ Unit tests"
    echo "  ✅ Integration tests"
    echo "  ✅ CLI binary"
    echo "  ✅ Documentation"
    exit 0
else
    print_error "The following checks failed:"
    for check in "${FAILED_CHECKS[@]}"; do
        echo "  • $check"
    done
    echo
    echo "🔧 Advanced debugging (for CI-specific issues):"
    echo "  ./scripts/ci-simulation.sh           # Enhanced CI environment simulation"
    echo "  ./scripts/debug-integration-tests.sh # Debug integration test failures"
    echo "  ./scripts/docker-ci-simulation.sh    # Docker-based exact CI replica"
    echo
    print_error "Please fix these issues before pushing!"
    exit 1
fi 