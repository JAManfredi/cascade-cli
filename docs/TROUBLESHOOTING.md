# 🔧 Troubleshooting Guide

This guide helps you diagnose and fix common issues with Cascade CLI.

## 🚨 **Quick Diagnostic**

Before diving into specific issues, run the built-in diagnostics:

```bash
# Run comprehensive health check
cc doctor

# Get detailed system information
cc doctor --verbose

# Show configuration
cc config list

# Check Git status
git status
```

---

## 📋 **Common Issues**

### **🔴 Installation & Setup**

#### **"command not found: cc"**

**Symptoms:**
```bash
$ cc --version
bash: cc: command not found
```

**Solutions:**

1. **Check if binary exists:**
   ```bash
   # If installed via cargo
   ls ~/.cargo/bin/cc
   
   # If built from source
   ls target/release/cc
   ```

2. **Fix PATH:**
   ```bash
   # Add to PATH temporarily
   export PATH="$HOME/.cargo/bin:$PATH"
   
   # Make permanent (bash)
   echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
   source ~/.bashrc
   
   # Make permanent (zsh)
   echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
   source ~/.zshrc
   ```

3. **Reinstall:**
   ```bash
   # Via cargo
   cargo install --path . --force
   
   # Or rebuild
   cargo build --release
   cp target/release/cc ~/.local/bin/
   ```

#### **Rust compilation errors**

**Symptoms:**
```
error: linking with `cc` failed: exit status: 1
```

**Solutions:**

1. **Update Rust:**
   ```bash
   rustup update
   rustc --version  # Should be 1.82+
   ```

2. **Install system dependencies:**
   ```bash
   # Ubuntu/Debian
   sudo apt update
   sudo apt install build-essential pkg-config libssl-dev
   
   # macOS
   xcode-select --install
   
   # CentOS/RHEL
   sudo yum groupinstall "Development Tools"
   sudo yum install openssl-devel
   ```

3. **Clear cache and rebuild:**
   ```bash
   cargo clean
   cargo build --release
   ```

#### **Permission denied errors**

**Symptoms:**
```bash
$ cc --version
Permission denied
```

**Solutions:**

1. **Fix permissions:**
   ```bash
   chmod +x ~/.cargo/bin/cc
   ```

2. **Install to user directory:**
   ```bash
   cargo install --path . --root ~/.local
   export PATH="$HOME/.local/bin:$PATH"
   ```

### **🔴 Configuration Issues**

#### **"Repository not initialized"**

**Symptoms:**
```
Error: Repository is not initialized for Cascade CLI
```

**Solutions:**

1. **Check if in Git repository:**
   ```bash
   git status
   # Should show repository status, not "not a git repository"
   ```

2. **Initialize Cascade CLI:**
   ```bash
   cc init --bitbucket-url https://your-bitbucket.com
   # Or use setup wizard
   cc setup
   ```

3. **Fix corrupted configuration:**
   ```bash
   rm -rf .cascade/
   cc init --force
   ```

#### **Bitbucket connection failures**

**Symptoms:**
```
Error: Failed to connect to Bitbucket: HTTP 401 Unauthorized
Error: Failed to connect to Bitbucket: Connection timeout
```

**Solutions:**

1. **Verify credentials:**
   ```bash
   cc config get bitbucket.token
   cc config get bitbucket.url
   
   # Test manually
   curl -H "Authorization: Bearer YOUR_TOKEN" \
        "https://your-bitbucket.com/rest/api/1.0/projects"
   ```

2. **Check Personal Access Token:**
   - Token must have **Repository Write** permissions
   - Token must not be expired
   - Check Bitbucket → Settings → Personal Access Tokens

3. **Network issues:**
   ```bash
   # Check DNS resolution
   nslookup your-bitbucket.com
   
   # Test connectivity
   ping your-bitbucket.com
   
   # Check proxy settings
   echo $HTTP_PROXY
   echo $HTTPS_PROXY
   ```

4. **Corporate firewall/proxy:**
   ```bash
   # Configure Git for proxy
   git config --global http.proxy http://proxy.company.com:8080
   git config --global https.proxy https://proxy.company.com:8080
   
   # Set environment variables
   export HTTP_PROXY=http://proxy.company.com:8080
   export HTTPS_PROXY=https://proxy.company.com:8080
   ```

#### **Invalid project/repository settings**

**Symptoms:**
```
Error: Project 'INVALID' not found
Error: Repository 'invalid-repo' not found in project 'PROJECT'
```

**Solutions:**

1. **List available projects:**
   ```bash
   curl -H "Authorization: Bearer YOUR_TOKEN" \
        "https://your-bitbucket.com/rest/api/1.0/projects" | jq '.values[].key'
   ```

2. **List repositories in project:**
   ```bash
   curl -H "Authorization: Bearer YOUR_TOKEN" \
        "https://your-bitbucket.com/rest/api/1.0/projects/PROJECT/repos" | jq '.values[].name'
   ```

3. **Auto-detect from Git remote:**
   ```bash
   git remote -v
   # Use the setup wizard to auto-detect
   cc setup --force
   ```

### **🔴 Stack Management Issues**

#### **"No active stack" errors**

**Symptoms:**
```
Error: No active stack found
```

**Solutions:**

1. **Check existing stacks:**
   ```bash
   cc stacks list
   ```

2. **Create or activate a stack:**
   ```bash
   # Create new stack
   cc stacks create my-feature --base main
   
   # Or activate existing stack
   cc stacks switch existing-stack-name
   ```

3. **Recover from corruption:**
   ```bash
   # Check stack metadata
   ls .cascade/stacks/
   
   # Validate stack integrity
   cc stacks validate
   
   # Force create new stack if needed
   cc stacks create recovery-stack --base main --force
   ```

#### **Stack synchronization failures**

**Symptoms:**
```
Error: Failed to sync stack: merge conflicts detected
Error: Base branch 'main' not found
```

**Solutions:**

1. **Resolve merge conflicts:**
   ```bash
   # Check conflict status
   git status
   
   # Resolve conflicts manually
   git add .
   cc stacks rebase --continue
   
   # Or abort and try different strategy
   cc stacks rebase --abort
   cc stacks sync --strategy merge
   ```

2. **Update base branch:**
   ```bash
   # Fetch latest changes
   git fetch origin
   
   # Ensure base branch exists
   git branch -r | grep origin/main
   
   # Update local base
   git checkout main
   git pull origin main
   
   # Try sync again
   cc stacks sync
   ```

3. **Stack corruption recovery:**
   ```bash
   # Backup current work
   git stash
   
   # Reset stack to known good state
   cc stack  # Note commit hashes
   git checkout main
   git pull origin main
   
   # Recreate stack manually
   cc stacks create recovery --base main
   git cherry-pick COMMIT_HASH_1
   cc stacks push
   git cherry-pick COMMIT_HASH_2
   cc stacks push
   ```

#### **Pull request creation failures**

**Symptoms:**
```
Error: Failed to create pull request: title cannot be empty
Error: Failed to create pull request: source branch not found
```

**Solutions:**

1. **Check commit exists:**
   ```bash
   git log --oneline -n 5
   cc stack
   ```

2. **Verify branch state:**
   ```bash
   # Check current branch
   git branch
   
   # Ensure commits are pushed to remote
   git push origin current-branch
   ```

3. **Manual PR creation:**
   ```bash
   # Get commit details
   cc stack
   
   # Create PR manually in Bitbucket UI
   # Then update stack metadata
   cc stacks submit --pr-id 123
   ```

### **🔴 Performance Issues**

#### **Slow operations in large repositories**

**Symptoms:**
- Commands taking > 30 seconds
- High memory usage
- Timeouts

**Solutions:**

1. **Optimize Git configuration:**
   ```bash
   git config core.preloadindex true
   git config core.fscache true
   git config gc.auto 256
   ```

2. **Adjust Cascade CLI settings:**
   ```bash
   cc config set performance.cache_size 500
   cc config set performance.parallel_operations false
   cc config set network.timeout 120
   ```

3. **Repository maintenance:**
   ```bash
   # Clean up Git repository
   git gc --aggressive
   git prune
   
   # Clear Cascade cache
   rm -rf .cascade/cache/
   ```

#### **High memory usage**

**Solutions:**

1. **Reduce cache size:**
   ```bash
   cc config set performance.cache_size 100
   ```

2. **Monitor memory usage:**
   ```bash
   # During operation
   top -p $(pgrep cc)
   
   # Check cache directory size
   du -sh .cascade/cache/
   ```

### **🔴 TUI (Terminal User Interface) Issues**

#### **TUI display problems**

**Symptoms:**
- Garbled characters
- No colors
- Layout issues

**Solutions:**

1. **Check terminal capabilities:**
   ```bash
   echo $TERM
   # Should be xterm-256color or similar
   
   # Test colors
   tput colors
   # Should be 256 or higher
   ```

2. **Fix terminal settings:**
   ```bash
   export TERM=xterm-256color
   
   # For tmux users
   export TERM=screen-256color
   ```

3. **Disable colors if needed:**
   ```bash
   cc config set ui.colors false
   cc tui
   ```

#### **TUI crashes or freezes**

**Solutions:**

1. **Update terminal:**
   ```bash
   # Check for terminal updates
   # Try different terminal: iTerm2, Alacritty, etc.
   ```

2. **Reset TUI settings:**
   ```bash
   cc config unset ui.tui_refresh_rate
   cc config unset ui.tui_theme
   ```

3. **Use alternative interface:**
   ```bash
   # Use CLI instead of TUI
   cc stacks list --verbose
   cc viz stack
   ```

### **🔴 Git Hooks Issues**

#### **Hooks not working**

**Symptoms:**
- Commits not auto-added to stack
- No pre-push validation

**Solutions:**

1. **Check hook installation:**
   ```bash
   cc hooks status
   ls -la .git/hooks/
   ```

2. **Verify permissions:**
   ```bash
   chmod +x .git/hooks/post-commit
   chmod +x .git/hooks/pre-push
   ```

3. **Reinstall hooks:**
   ```bash
   cc hooks uninstall
   cc hooks install --force
   ```

#### **Hook conflicts**

**Symptoms:**
```
Error: Existing hook found, use --force to overwrite
```

**Solutions:**

1. **Backup existing hooks:**
   ```bash
   cp .git/hooks/post-commit .git/hooks/post-commit.backup
   ```

2. **Force install:**
   ```bash
   cc hooks install --force
   ```

3. **Manual integration:**
   ```bash
   # Edit existing hook to call Cascade CLI
   echo "cc stacks push --auto || true" >> .git/hooks/post-commit
   ```

---

## 🔍 **Advanced Debugging**

### **Enable Debug Logging**

```bash
# Set debug level
export CASCADE_LOG_LEVEL=debug

# Run command with debug output
cc stacks push

# Check logs
tail -f ~/.cascade/logs/cascade.log
```

### **Capture System Information**

```bash
# Full diagnostic report
cc doctor --verbose > cascade-debug.txt

# Add system info
echo "=== System Info ===" >> cascade-debug.txt
uname -a >> cascade-debug.txt
git --version >> cascade-debug.txt
rustc --version >> cascade-debug.txt

# Add configuration
echo "=== Configuration ===" >> cascade-debug.txt
cc config list >> cascade-debug.txt

# Add recent logs
echo "=== Recent Logs ===" >> cascade-debug.txt
tail -50 ~/.cascade/logs/cascade.log >> cascade-debug.txt
```

### **Network Debugging**

```bash
# Test Bitbucket API manually
curl -v -H "Authorization: Bearer YOUR_TOKEN" \
     "https://your-bitbucket.com/rest/api/1.0/projects"

# Check DNS resolution
dig your-bitbucket.com

# Test with different timeout
cc config set network.timeout 300
```

### **Repository State Analysis**

```bash
# Check Git integrity
git fsck --full

# Analyze repository size
git count-objects -vH

# Check remote configuration
git remote show origin

# Analyze stack metadata
find .cascade/ -name "*.json" -exec cat {} \;
```

---

## 🛠️ **Recovery Procedures**

### **Complete Reset**

If everything is broken:

```bash
# 1. Backup important work
git stash
git branch backup-$(date +%Y%m%d)

# 2. Remove Cascade configuration
rm -rf .cascade/

# 3. Reinitialize
cc setup

# 4. Recreate stacks manually
cc stacks create recovery --base main
# Cherry-pick commits as needed
```

### **Stack Recovery**

For corrupted stack metadata:

```bash
# 1. Export stack information
cc stack > stack-backup.txt

# 2. Note commit hashes and PR IDs

# 3. Delete corrupted stack
cc stacks delete problematic-stack --force

# 4. Recreate with same commits
cc stacks create recovered-stack --base main
git cherry-pick HASH1
cc stacks push
git cherry-pick HASH2  
cc stacks push

# 5. Reconnect to existing PRs
cc config set stack.recovered-stack.pr.1 PR_ID_1
cc config set stack.recovered-stack.pr.2 PR_ID_2
```

### **Configuration Recovery**

For broken configuration:

```bash
# 1. Backup current config
cp .cascade/config.toml .cascade/config.toml.backup

# 2. Reset to defaults
cc config reset

# 3. Reconfigure step by step
cc config set bitbucket.url "https://your-bitbucket.com"
cc config set bitbucket.project "PROJECT"
cc config set bitbucket.repository "repo"
cc config set bitbucket.token "YOUR_TOKEN"

# 4. Test configuration
cc doctor
```

---

## 📞 **Getting Help**

### **Self-Service Resources**

1. **Built-in help:**
   ```bash
   cc --help
   cc stack --help
   cc stacks create --help
   ```

2. **Diagnostics:**
   ```bash
   cc doctor --verbose
   ```

3. **Documentation:**
   - [User Manual](./USER_MANUAL.md) - Complete command reference
   - [Installation Guide](./INSTALLATION.md) - Setup instructions
   - [Configuration Guide](./CONFIGURATION.md) - Settings reference

### **Community Support**

1. **Search existing issues:**
   - [GitHub Issues](https://github.com/JAManfredi/cascade-cli/issues)
   - Use search terms: error message, symptom keywords

2. **Community discussions:**
   - [GitHub Discussions](https://github.com/JAManfredi/cascade-cli/discussions)
   - Stack Overflow (tag: cascade-cli)

3. **Create new issue:**
   Include this information:
   ```bash
   # System information
   cc doctor --verbose
   
   # Error reproduction steps
   # Expected vs actual behavior
   # Configuration (sanitized)
   cc config list | sed 's/token = .*/token = [REDACTED]/'
   ```

### **Enterprise Support**

For enterprise users:

1. **Internal support channels:**
   - Check your company's internal documentation
   - Contact IT support for network/proxy issues

2. **Configuration templates:**
   - Ask your team lead for standard configuration
   - Check if there's a company-specific setup guide

---

## 🎯 **Prevention Tips**

### **Best Practices**

1. **Regular maintenance:**
   ```bash
   # Weekly repository cleanup
   git gc
   cc doctor
   
   # Monthly cache cleanup
   rm -rf .cascade/cache/
   ```

2. **Configuration backup:**
   ```bash
   # Backup configuration
   cp .cascade/config.toml ~/.cascade-config-backup.toml
   
   # Version control team config
   git add .cascade/config.toml
   git commit -m "Add team Cascade CLI configuration"
   ```

3. **Monitor health:**
   ```bash
   # Regular health checks
   cc doctor | grep -E "(ERROR|WARN)"
   
   # Check for updates
   cargo install cascade-cli --force
   ```

### **Common Mistakes to Avoid**

1. **Don't manually force push to shared branches** (Cascade CLI handles force pushes safely during rebase)
2. **Don't ignore merge conflicts**  
3. **Don't work on multiple stacks simultaneously without switching**
4. **Don't delete .cascade/ directory unless troubleshooting**
5. **Don't commit sensitive information in configuration**

---

## ⚡ **Smart Force Push Issues**

### **Force Push Failed**

If force push operations fail during rebase:

```bash
# Error: Force push rejected
# Solution 1: Check branch protection rules
git ls-remote --heads origin | grep your-branch

# Solution 2: Verify you have push permissions
git remote show origin

# Solution 3: Force push manually as fallback
git checkout original-branch
git reset --hard versioned-branch
git push --force-with-lease origin original-branch
```

### **PR Links Broken After Rebase**

If PRs don't update correctly:

```bash
# Check if PRs still exist
cc stacks prs --verbose

# Manually update PR if needed  
cc config set stack.STACK_NAME.pr.INDEX PR_ID

# Re-submit if PR was closed
cc stacks submit INDEX --title "Updated after rebase"
```

### **Versioned Branches Accumulating**

Clean up temporary rebase branches:

```bash
# List versioned branches
git branch | grep -E '.*-v[0-9]+$'

# Clean up old versions (keep latest)
git branch | grep -E '.*-v[1-9][0-9]*$' | xargs -n 1 git branch -D

# Or use cascade cleanup
cc stack cleanup --remove-versioned-branches
```

### **Force Push Safety Concerns**

Understanding when force pushes are safe:

```bash
# Cascade CLI force pushes are safe because:
# 1. Only affects your feature branches (never main/develop)
# 2. Validates existing PRs before pushing
# 3. Creates backup branches before operations
# 4. Uses --force-with-lease for additional safety

# Check backup branches exist
git branch | grep -E '.*-v[0-9]+$'

# Manually verify safety
git log --oneline origin/your-branch..your-branch-v2
```

---

## 📊 **Error Code Reference**

| Code | Meaning | Common Solutions |
|------|---------|------------------|
| E001 | Configuration missing | Run `cc init` or `cc setup` |
| E002 | Git repository not found | Ensure you're in a Git repository |
| E003 | Bitbucket connection failed | Check credentials and network |
| E004 | Stack not found | Use `cc stacks list` to see available stacks |
| E005 | Merge conflict detected | Resolve conflicts and run `cc stacks rebase --continue` |
| E006 | Invalid commit hash | Check commit exists with `git log` |
| E007 | Permission denied | Check file permissions and access rights |
| E008 | Network timeout | Increase timeout with `cc config set network.timeout 120` |

---

*If your issue isn't covered here, please [create an issue](https://github.com/JAManfredi/cascade-cli/issues/new) with detailed information about your problem.* 