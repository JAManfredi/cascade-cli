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

print_header "Docker-based CI Environment Simulation"
echo "This script creates an exact replica of the GitHub Actions CI environment"
echo "using Docker to catch environment-specific issues that don't reproduce locally."
echo

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed or not available in PATH"
    echo "Please install Docker to use this CI simulation."
    echo "Visit: https://docs.docker.com/get-docker/"
    exit 1
fi

# Check if Docker is running
if ! docker info &> /dev/null; then
    print_error "Docker is not running"
    echo "Please start Docker Desktop or the Docker daemon and try again."
    exit 1
fi

# Parse command line arguments
RUST_VERSION="stable"
OS_VERSION="ubuntu-latest"
KEEP_CONTAINER=false
INTERACTIVE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --rust-version)
            RUST_VERSION="$2"
            shift 2
            ;;
        --os)
            OS_VERSION="$2"
            shift 2
            ;;
        --keep-container)
            KEEP_CONTAINER=true
            shift
            ;;
        --interactive)
            INTERACTIVE=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  --rust-version VERSION  Rust version to use (default: stable)"
            echo "  --os VERSION           OS version (ubuntu-latest, ubuntu-20.04, etc.)"
            echo "  --keep-container       Keep container after test for debugging"
            echo "  --interactive          Drop into interactive shell after tests"
            echo "  -h, --help            Show this help message"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Map OS versions to Docker images
case "$OS_VERSION" in
    ubuntu-latest|ubuntu-22.04)
        DOCKER_IMAGE="ubuntu:22.04"
        ;;
    ubuntu-20.04)
        DOCKER_IMAGE="ubuntu:20.04"
        ;;
    ubuntu-18.04)
        DOCKER_IMAGE="ubuntu:18.04"
        ;;
    *)
        print_warning "Unsupported OS version: $OS_VERSION, using ubuntu-22.04"
        DOCKER_IMAGE="ubuntu:22.04"
        ;;
esac

print_step "Using Docker image: $DOCKER_IMAGE"
print_step "Using Rust version: $RUST_VERSION"

# Generate container name
CONTAINER_NAME="cascade-ci-$(date +%s)"

# Create Dockerfile content
cat > .docker-ci-simulation/Dockerfile << EOF
FROM $DOCKER_IMAGE

# Install system dependencies (matching GitHub Actions runner)
RUN apt-get update && apt-get install -y \\
    curl \\
    build-essential \\
    pkg-config \\
    libssl-dev \\
    git \\
    ca-certificates \\
    && rm -rf /var/lib/apt/lists/*

# Install Rust (matching GitHub Actions dtolnay/rust-toolchain)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain $RUST_VERSION
ENV PATH="/root/.cargo/bin:\$PATH"

# Install Rust components
RUN rustup component add rustfmt clippy

# Set up Git configuration (matching CI)
RUN git config --global user.name "CI Simulation" && \\
    git config --global user.email "ci@local.dev" && \\
    git config --global init.defaultBranch main

# Set up environment variables (matching CI)
ENV CARGO_TERM_COLOR=always
ENV CI=true
ENV GITHUB_ACTIONS=true
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Create working directory
WORKDIR /workspace

# Copy source code
COPY . .

# Create the test script
RUN cat > run-ci-tests.sh << 'SCRIPT_EOF'
#!/bin/bash
set -e

echo "🏗️  Starting CI Environment Simulation"
echo "Docker image: $DOCKER_IMAGE"
echo "Rust version: \$(rustc --version)"
echo "Git version: \$(git --version)"
echo

echo "📦 Installing additional tools..."
if command -v cargo-audit &> /dev/null || cargo install cargo-audit; then
    echo "✅ cargo-audit available"
else
    echo "⚠️  cargo-audit installation failed"
fi

echo
echo "🧹 Cleaning workspace..."
cargo clean

echo
echo "🔍 Running CI checks..."

# Format check
echo "Checking code formatting..."
cargo fmt --all -- --check

# Clippy check
echo "Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

# Build
echo "Building project..."
cargo build --verbose

# Unit tests
echo "Running unit tests..."
cargo test --lib --verbose

# Build release binary
echo "Building release binary..."
cargo build --release

# Integration tests (with CI-specific environment)
echo "Running integration tests..."
export RUST_TEST_THREADS=1
export INTEGRATION_TEST_CONCURRENCY=1
export TEST_TIMEOUT=300
timeout 1800 cargo test --test '*' --verbose || {
    echo "❌ Integration tests failed or timed out"
    exit 1
}

# CLI binary test
echo "Testing CLI binary..."
./target/release/cc --help

# Documentation
echo "Checking documentation..."
cargo doc --no-deps --document-private-items
cargo test --doc

# Security audit (if available)
if command -v cargo-audit &> /dev/null; then
    echo "Running security audit..."
    cargo audit || echo "⚠️  Security audit found issues"
fi

echo
echo "✅ All CI checks completed successfully!"

SCRIPT_EOF

chmod +x run-ci-tests.sh

CMD ["/bin/bash", "run-ci-tests.sh"]
EOF

# Ensure the Docker context directory exists
mkdir -p .docker-ci-simulation

print_step "Building Docker CI environment..."
if docker build -f .docker-ci-simulation/Dockerfile -t cascade-ci-env .; then
    print_success "Docker CI environment built successfully"
else
    print_error "Failed to build Docker CI environment"
    exit 1
fi

print_step "Running CI tests in Docker container..."

# Prepare Docker run command
docker_cmd="docker run --name $CONTAINER_NAME"

if [ "$INTERACTIVE" = true ]; then
    docker_cmd="$docker_cmd -it"
fi

# Add volume mount for source code (for development iteration)
docker_cmd="$docker_cmd -v $(pwd):/workspace"

# Run the container
if eval "$docker_cmd cascade-ci-env"; then
    print_success "Docker CI simulation completed successfully!"
    CI_SUCCESS=true
else
    print_error "Docker CI simulation failed"
    CI_SUCCESS=false
fi

# Interactive debugging session
if [ "$INTERACTIVE" = true ] || [ "$CI_SUCCESS" = false ]; then
    print_step "Starting interactive debugging session..."
    echo "You can now debug the issues inside the CI environment."
    echo "Type 'exit' to leave the container."
    docker exec -it "$CONTAINER_NAME" /bin/bash || true
fi

# Cleanup
if [ "$KEEP_CONTAINER" = false ]; then
    print_step "Cleaning up container..."
    docker rm -f "$CONTAINER_NAME" &>/dev/null || true
else
    print_warning "Container '$CONTAINER_NAME' kept for debugging"
    echo "To remove later: docker rm -f $CONTAINER_NAME"
fi

# Cleanup Docker build context
rm -rf .docker-ci-simulation

if [ "$CI_SUCCESS" = true ]; then
    print_success "Docker CI simulation passed!"
    echo
    echo "🎯 Your code should pass CI on GitHub Actions."
    echo "   The Docker environment closely matches the CI environment."
else
    print_error "Docker CI simulation failed!"
    echo
    echo "💡 The failures in Docker likely indicate real CI issues."
    echo "   Fix these issues before pushing to avoid CI failures."
    echo
    echo "🔧 Debug suggestions:"
    echo "   $0 --interactive  # Debug interactively"
    echo "   $0 --keep-container  # Keep container for later debugging"
    echo "   docker logs $CONTAINER_NAME  # View full logs"
fi

exit $([[ "$CI_SUCCESS" = true ]] && echo 0 || echo 1) 