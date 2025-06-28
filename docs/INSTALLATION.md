# 🚀 Installation Guide

This guide covers installing Cascade CLI on different platforms and environments.

## 📋 **System Requirements**

### **Minimum Requirements**
- **Operating System**: macOS 10.15+, Linux (Ubuntu 18.04+), Windows 10+
- **Git**: Version 2.20+ installed and configured
- **Rust**: 1.70+ (for building from source)
- **Memory**: 50MB RAM minimum
- **Disk**: 100MB free space

### **Recommended Environment**
- **Bitbucket Server/Cloud** access with Personal Access Token
- **Terminal** with 256 color support
- **Shell**: bash, zsh, or fish for completions

---

## 🔧 **Installation Methods**

### **Option 1: From Source (Recommended)**

#### **Prerequisites**
```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version
```

#### **Build and Install**
```bash
# Clone repository
git clone https://github.com/JAManfredi/cascade-cli.git
cd cascade-cli

# Build release binary
cargo build --release

# Install to ~/.cargo/bin (automatically in PATH)
cargo install --path .

# Or manually add to PATH
export PATH="$PWD/target/release:$PATH"
echo 'export PATH="'$PWD'/target/release:$PATH"' >> ~/.bashrc
```

#### **Verify Installation**
```bash
cc --version
cc doctor  # Run health check
```

### **Option 2: Pre-built Binaries** *(Recommended)*

#### **macOS**
```bash
# Intel Macs (x64)
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-macos-x64.tar.gz | tar -xz
sudo mv cc /usr/local/bin/

# Apple Silicon Macs (ARM64)  
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-macos-arm64.tar.gz | tar -xz
sudo mv cc /usr/local/bin/

# Auto-detect architecture
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-macos-$(uname -m | sed 's/x86_64/x64/;s/arm64/arm64/').tar.gz | tar -xz
sudo mv cc /usr/local/bin/
```

#### **Linux**
```bash
# x64 (Intel/AMD)
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-linux-x64.tar.gz | tar -xz
sudo mv cc /usr/local/bin/

# ARM64
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-linux-arm64.tar.gz | tar -xz  
sudo mv cc /usr/local/bin/

# Auto-detect architecture
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-linux-$(uname -m | sed 's/x86_64/x64/;s/aarch64/arm64/').tar.gz | tar -xz
sudo mv cc /usr/local/bin/
```

#### **Windows**
```powershell
# x64 (Intel/AMD)
Invoke-WebRequest -Uri "https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-windows-x64.exe.zip" -OutFile "cc.zip"
Expand-Archive -Path "cc.zip" -DestinationPath "$env:USERPROFILE\bin\"

# ARM64  
Invoke-WebRequest -Uri "https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-windows-arm64.exe.zip" -OutFile "cc.zip"
Expand-Archive -Path "cc.zip" -DestinationPath "$env:USERPROFILE\bin\"

# Add to PATH if needed
$env:PATH += ";$env:USERPROFILE\bin"
```

### **Option 3: Package Managers** *(Planned)*

#### **macOS - Homebrew**
```bash
brew tap JAManfredi/cascade-cli
brew install cascade-cli
```

#### **Rust - Cargo**
```bash
cargo install cascade-cli
```

---

## ⚙️ **Post-Installation Setup**

### **1. Verify Installation**
```bash
# Check version
cc --version

# Run system diagnostics
cc doctor

# Test help system
cc --help
```

### **2. Shell Completions**
```bash
# Auto-detect and install for your shell
cc completions install

# Manual installation
cc completions generate bash > ~/.local/share/bash-completion/completions/cc
cc completions generate zsh > ~/.zsh/completions/_cc
cc completions generate fish > ~/.config/fish/completions/cc.fish
```

### **3. First-Time Configuration**
```bash
# Run interactive setup wizard
cc setup

# Manual configuration (if preferred)
cd your-git-repository
cc init --bitbucket-url https://bitbucket.your-company.com
```

---

## 🏢 **Enterprise/Corporate Installation**

### **Behind Corporate Firewall**
```bash
# Configure Git for corporate proxy
git config --global http.proxy http://proxy.company.com:8080
git config --global https.proxy https://proxy.company.com:8080

# Build with proxy settings
export HTTP_PROXY=http://proxy.company.com:8080
export HTTPS_PROXY=https://proxy.company.com:8080
cargo build --release
```

### **Custom Certificate Authority**
```bash
# Add corporate CA certificates
export SSL_CERT_FILE=/path/to/corporate-ca-bundle.crt
export SSL_CERT_DIR=/path/to/cert/directory

# Or configure in Git
git config --global http.sslCAInfo /path/to/corporate-ca-bundle.crt
```

### **Restricted Environments**
```bash
# Air-gapped installation
# 1. Download source on internet-connected machine
# 2. Transfer to target environment
# 3. Build offline

cargo build --release --offline
```

---

## 🐳 **Container Installation**

### **Docker**
```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y git ca-certificates
COPY --from=builder /app/target/release/cc /usr/local/bin/
ENTRYPOINT ["cc"]
```

### **Docker Usage**
```bash
# Build image
docker build -t cascade-cli .

# Run in project directory
docker run -v $(pwd):/workspace -w /workspace cascade-cli status

# Create alias for convenience
alias cc='docker run -v $(pwd):/workspace -w /workspace cascade-cli'
```

---

## 🔧 **Development Installation**

### **For Contributors**
```bash
# Clone with development dependencies
git clone https://github.com/JAManfredi/cascade-cli.git
cd cascade-cli

# Install development tools
rustup component add clippy rustfmt
cargo install cargo-watch cargo-audit

# Run in development mode
cargo run -- --help

# Auto-rebuild on changes
cargo watch -x run
```

### **IDE Setup**

#### **VS Code**
```json
{
  "rust-analyzer.cargo.allFeatures": true,
  "rust-analyzer.checkOnSave.command": "clippy"
}
```

#### **IntelliJ IDEA**
- Install Rust plugin
- Configure Cargo toolchain path
- Enable Clippy integration

---

## 🚨 **Troubleshooting**

### **Common Issues**

#### **"command not found: cc"**
```bash
# Verify PATH includes binary location
echo $PATH
which cc

# Add to PATH if missing
export PATH="$HOME/.cargo/bin:$PATH"
# Make permanent in ~/.bashrc or ~/.zshrc
```

#### **Rust compilation errors**
```bash
# Update Rust toolchain
rustup update

# Clear cargo cache
cargo clean

# Verify dependencies
cargo check
```

#### **Git integration issues**
```bash
# Verify Git installation
git --version

# Check repository status
git status

# Ensure proper Git configuration
git config --list
```

#### **Permission denied**
```bash
# Fix binary permissions
chmod +x /path/to/cc

# Install to user directory instead of system
cargo install --path . --root ~/.local
export PATH="$HOME/.local/bin:$PATH"
```

### **Platform-Specific Issues**

#### **macOS**
```bash
# Xcode Command Line Tools required
xcode-select --install

# macOS Gatekeeper issues
sudo spctl --add /usr/local/bin/cc
```

#### **Linux**
```bash
# Missing system dependencies
sudo apt update
sudo apt install build-essential git pkg-config libssl-dev

# For CentOS/RHEL
sudo yum groupinstall "Development Tools"
sudo yum install git openssl-devel
```

#### **Windows**
```powershell
# Visual Studio Build Tools required
# Download from: https://visualstudio.microsoft.com/visual-cpp-build-tools/

# WSL recommended for better experience
wsl --install
```

---

## 📊 **Performance Optimization**

### **Large Repositories**
```bash
# Increase Git performance
git config core.preloadindex true
git config core.fscache true
git config gc.auto 256

# Configure Cascade CLI
cc config set performance.cache_size 1000
cc config set performance.parallel_operations true
```

### **Network Optimization**
```bash
# Configure timeouts for slow networks
cc config set network.timeout 60
cc config set network.retry_attempts 3

# Use compression
git config core.compression 9
```

---

## 🔄 **Updating**

### **From Source**
```bash
cd cascade-cli
git pull origin main
cargo build --release
cargo install --path . --force
```

### **Package Manager Updates**
```bash
# Homebrew
brew upgrade cascade-cli

# Cargo
cargo install cascade-cli --force
```

---

## ❌ **Uninstallation**

### **Complete Removal**
```bash
# Remove binary
rm /usr/local/bin/cc
# or
cargo uninstall cascade-cli

# Remove configuration
rm -rf ~/.cascade/

# Remove completions
rm ~/.local/share/bash-completion/completions/cc
rm ~/.zsh/completions/_cc
rm ~/.config/fish/completions/cc.fish

# Remove Git hooks (per repository)
cd your-repository
cc hooks uninstall
```

---

## 📞 **Support**

If you encounter installation issues:

1. **Check [Troubleshooting Guide](./TROUBLESHOOTING.md)**
2. **Run `cc doctor` for diagnostics**
3. **Search [GitHub Issues](https://github.com/JAManfredi/cascade-cli/issues)**
4. **Create new issue with system details**

### **System Information for Bug Reports**
```bash
# Gather system info
cc doctor --verbose
rustc --version
git --version
uname -a  # Linux/macOS
systeminfo  # Windows
``` 