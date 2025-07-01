#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${CYAN}=================================================${NC}"
    echo -e "${CYAN} $1"
    echo -e "${CYAN}=================================================${NC}"
}

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

print_header "Integration Test Debugging"
echo "This script helps identify the root causes of integration test failures."
echo

# Set up environment
export RUST_BACKTRACE=full
export RUST_LOG=debug
export CI=true

# Build binary if not exists
if [[ ! -f "target/release/ca" ]]; then
    print_step "Building release binary for integration tests"
    cargo build --release
fi

# Run individual test modules to isolate failures
print_step "Running individual integration test modules"

MODULES=(
    "config_management_tests"
    "end_to_end_tests"
    "multi_stack_tests"
    "network_failure_tests"
    "squash_and_push_tests"
    "bitbucket_api_tests"
    "test_helpers"
)

FAILED_MODULES=()

for module in "${MODULES[@]}"; do
    echo
    print_step "Testing module: $module"
    
    # Run the module with detailed output
    if cargo test --test integration_tests "integration::${module}" --verbose -- --nocapture; then
        print_success "$module passed"
    else
        print_error "$module failed"
        FAILED_MODULES+=("$module")
    fi
done

echo
print_header "Detailed Analysis of Failed Modules"

if [ ${#FAILED_MODULES[@]} -eq 0 ]; then
    print_success "All integration test modules passed!"
    exit 0
fi

for module in "${FAILED_MODULES[@]}"; do
    echo
    print_step "Analyzing failures in: $module"
    
    case "$module" in
        "config_management_tests")
            echo "Common issues in config_management_tests:"
            echo "  • File permission errors on different platforms"
            echo "  • Concurrent access race conditions"
            echo "  • Config file corruption handling"
            echo "  • Directory cleanup issues"
            echo
            echo "Debug suggestions:"
            echo "  1. Check file permissions: ls -la .cascade/"
            echo "  2. Run with strace/dtrace to see file operations"
            echo "  3. Reduce concurrency: export INTEGRATION_TEST_CONCURRENCY=1"
            ;;
            
        "end_to_end_tests")
            echo "Common issues in end_to_end_tests:"
            echo "  • Git repository state not properly isolated"
            echo "  • Binary not found or wrong binary used"
            echo "  • Timeout issues in CI environment"
            echo "  • Working directory changes between tests"
            echo
            echo "Debug suggestions:"
            echo "  1. Verify binary path: ls -la target/release/ca"
            echo "  2. Check git config: git config --list"
            echo "  3. Increase timeouts: export TEST_TIMEOUT=600"
            ;;
            
        "multi_stack_tests")
            echo "Common issues in multi_stack_tests:"
            echo "  • Stack metadata not properly isolated between tests"
            echo "  • Concurrent operations causing state corruption"
            echo "  • Cleanup not happening between tests"
            echo
            echo "Debug suggestions:"
            echo "  1. Check for leftover .cascade directories"
            echo "  2. Run tests individually: cargo test --test integration_tests test_multi_stack_creation_and_switching"
            echo "  3. Monitor temp directories: watch 'find /tmp -name \"*cascade*\" -type d'"
            ;;
            
        "network_failure_tests")
            echo "Common issues in network_failure_tests:"
            echo "  • Network timeouts in CI environment"
            echo "  • Mock server setup/teardown issues"
            echo "  • Rate limiting simulation problems"
            echo
            echo "Debug suggestions:"
            echo "  1. Check network connectivity: curl -I https://httpbin.org/status/200"
            echo "  2. Verify mock server: netstat -an | grep :1080"
            echo "  3. Increase network timeouts"
            ;;
            
        "squash_and_push_tests")
            echo "Common issues in squash_and_push_tests:"
            echo "  • Git repository state pollution"
            echo "  • Commit counting logic failures"
            echo "  • Branch state not properly reset"
            echo
            echo "Debug suggestions:"
            echo "  1. Check git status: git status --porcelain"
            echo "  2. Verify commit history: git log --oneline -10"
            echo "  3. Check branch state: git branch -a"
            ;;
            
        "bitbucket_api_tests")
            echo "Common issues in bitbucket_api_tests:"
            echo "  • Mock API server issues"
            echo "  • Authentication header problems"
            echo "  • JSON serialization/deserialization"
            echo
            echo "Debug suggestions:"
            echo "  1. Check mock server logs"
            echo "  2. Verify API endpoints: curl -v http://localhost:1080/test"
            echo "  3. Test JSON parsing manually"
            ;;
            
        "test_helpers")
            echo "Common issues in test_helpers:"
            echo "  • Timeout wrapper not working correctly"
            echo "  • Parallel operations causing interference"
            echo "  • Test fixture cleanup issues"
            echo
            echo "Debug suggestions:"
            echo "  1. Check system load: top -n1 | head -5"
            echo "  2. Monitor file descriptors: lsof -p $$"
            echo "  3. Reduce parallelism: export RUST_TEST_THREADS=1"
            ;;
    esac
    
    # Run the specific failing module with maximum verbosity
    echo
    print_step "Running $module with maximum verbosity"
    echo "Command: cargo test --test integration_tests \"integration::${module}\" --verbose -- --nocapture --test-threads=1"
    echo
    
    # Optional: Run with timeout to prevent hanging
    timeout 300 cargo test --test integration_tests "integration::${module}" --verbose -- --nocapture --test-threads=1 || true
done

echo
print_header "Environment Debugging Information"

print_step "System Information"
echo "OS: $(uname -a)"
echo "Rust: $(rustc --version)"
echo "Cargo: $(cargo --version)"
echo "Git: $(git --version)"
echo "Shell: $SHELL"
echo "PWD: $(pwd)"

print_step "Binary Information"
if [[ -f "target/release/ca" ]]; then
    echo "Binary exists: target/release/ca"
    echo "Binary size: $(stat -f%z target/release/ca 2>/dev/null || stat -c%s target/release/ca 2>/dev/null || echo 'unknown')"
    echo "Binary permissions: $(ls -la target/release/ca)"
    echo "Binary test: $(./target/release/ca --version 2>&1 || echo 'FAILED')"
else
    print_error "Binary not found: target/release/ca"
fi

print_step "Git Configuration"
echo "Git user: $(git config user.name) <$(git config user.email)>"
echo "Git branch: $(git branch --show-current)"
echo "Git status: $(git status --porcelain | wc -l) modified files"

print_step "Environment Variables"
echo "CI: ${CI:-not set}"
echo "RUST_LOG: ${RUST_LOG:-not set}"
echo "RUST_BACKTRACE: ${RUST_BACKTRACE:-not set}"
echo "TEST_TIMEOUT: ${TEST_TIMEOUT:-not set}"
echo "INTEGRATION_TEST_CONCURRENCY: ${INTEGRATION_TEST_CONCURRENCY:-not set}"

print_step "Resource Usage"
echo "Disk space: $(df -h . | tail -1)"
echo "Memory: $(free -h 2>/dev/null || vm_stat 2>/dev/null | head -5 || echo 'Memory info not available')"
echo "Load average: $(uptime)"

print_step "Temporary Files"
echo "Temp directories: $(find /tmp -name "*cascade*" -type d 2>/dev/null | wc -l)"
echo "Open files: $(lsof -p $$ 2>/dev/null | wc -l || echo 'lsof not available')"

echo
print_header "Recommendations"

if [ ${#FAILED_MODULES[@]} -gt 0 ]; then
    echo "Based on the failing modules, here are the recommended actions:"
    echo
    echo "1. 🧹 Clean up your environment:"
    echo "   cargo clean"
    echo "   rm -rf /tmp/*cascade*"
    echo "   killall ca 2>/dev/null || true"
    echo
    echo "2. 🔧 Try running with restricted resources (like CI):"
    echo "   export RUST_TEST_THREADS=1"
    echo "   export INTEGRATION_TEST_CONCURRENCY=1"
    echo "   export TEST_TIMEOUT=300"
    echo
    echo "3. 🐛 Run individual failing tests:"
    for module in "${FAILED_MODULES[@]}"; do
        echo "   cargo test --test integration_tests \"integration::${module}\" -- --nocapture"
    done
    echo
    echo "4. 🔍 Use Docker to simulate exact CI environment:"
    echo "   ./scripts/docker-ci-simulation.sh"
    echo
    echo "5. 📊 Monitor system resources during test run:"
    echo "   watch -n1 'ps aux | grep ca; find /tmp -name \"*cascade*\" -type d'"
fi 