# 📚 User Manual

Complete reference guide for Cascade CLI commands, workflows, and advanced usage.

## 📖 **Table of Contents**

1. [Core Concepts](#core-concepts)
2. [Command Reference](#command-reference)
3. [Workflow Patterns](#workflow-patterns)
4. [Advanced Usage](#advanced-usage)
5. [Configuration](#configuration)
6. [Troubleshooting](#troubleshooting)

---

## 🧭 **Core Concepts**

### **What is a Stack?**
A **stack** is a logical grouping of related commits that represent incremental progress toward a larger feature or fix. Each commit in the stack can be submitted as a separate pull request while maintaining dependencies.

```
📚 Stack: "user-authentication"
├── Commit 1: "Add login endpoint"        → PR #123
├── Commit 2: "Add password validation"   → PR #124 (depends on #123)
└── Commit 3: "Add password reset"        → PR #125 (depends on #124)
```

### **Key Benefits**
- **Faster Reviews**: Small, focused PRs are easier to review
- **Parallel Development**: Work on multiple features simultaneously
- **Better Quality**: Incremental feedback improves code quality
- **Reduced Conflicts**: Frequent integration prevents merge hell

### **Stack Lifecycle**
1. **Create** - Initialize a new stack with a base branch
2. **Push** - Add commits to the stack
3. **Submit** - Create pull requests for stack entries
4. **Sync** - Update stack when dependencies change
5. **Pop** - Remove completed entries from the stack

---

## 📖 **Command Reference**

### **🎯 Core Commands**

#### **`cc init`** - Initialize Repository
Initialize Cascade CLI in a Git repository.

```bash
cc init [OPTIONS]

# Options:
--bitbucket-url <URL>     # Bitbucket Server URL
--project <PROJECT>       # Project key
--repository <REPO>       # Repository name
--force                   # Overwrite existing configuration
```

**Examples:**
```bash
# Interactive initialization
cc init

# Manual configuration
cc init --bitbucket-url https://bitbucket.company.com --project DEV --repository my-app

# Force reconfiguration
cc init --force
```

#### **`cc setup`** - Interactive Setup Wizard
Guided configuration for first-time users.

```bash
cc setup [OPTIONS]

# Options:
--force                   # Force reconfiguration if already initialized
```

**Features:**
- Auto-detects Git remotes
- Configures Bitbucket settings
- Tests connections
- Installs shell completions
- Validates Personal Access Tokens

### **📚 Stack Management**

#### **`cc stack create`** - Create New Stack
Create a new stack for organizing related commits.

```bash
cc stack create <NAME> [OPTIONS]

# Options:
--base <BRANCH>           # Base branch (default: current branch)
--description <DESC>      # Stack description
--activate               # Activate after creation (default: true)
```

**Examples:**
```bash
# Basic stack creation
cc stack create feature-auth --base develop

# With description
cc stack create fix-performance --base main --description "Database query optimizations"

# Create without activating
cc stack create future-feature --base develop --no-activate
```

#### **`cc stack list`** - List All Stacks
Display all stacks with their status and information.

```bash
cc stack list [OPTIONS]

# Options:
--verbose, -v            # Show detailed information
--active                 # Show only active stack
--format <FORMAT>        # Output format (name, id, status)
```

**Examples:**
```bash
# Simple list
cc stack list

# Detailed view
cc stack list --verbose

# Only active stack
cc stack list --active

# Custom format
cc stack list --format status
```

#### **`cc stack show`** - Display Stack Details
Show detailed information about a specific stack.

```bash
cc stack show [NAME]

# Arguments:
[NAME]                   # Stack name (defaults to active stack)
```

**Output includes:**
- Stack metadata (name, description, base branch)
- All stack entries with commit details
- Pull request status and links
- Dependency information

#### **`cc stack switch`** - Activate Stack
Switch to a different stack, making it the active stack.

```bash
cc stack switch <NAME>

# Arguments:
<NAME>                   # Stack name to activate
```

**Examples:**
```bash
cc stack switch feature-auth
cc stack switch fix-bugs
```

#### **`cc stack delete`** - Remove Stack
Delete a stack and optionally its associated branches.

```bash
cc stack delete <NAME> [OPTIONS]

# Options:
--force                  # Skip confirmation prompt
--keep-branches         # Keep associated branches
```

**Examples:**
```bash
# With confirmation
cc stack delete old-feature

# Force deletion
cc stack delete temp-stack --force

# Delete but keep branches
cc stack delete feature-x --keep-branches
```

### **📤 Stack Operations**

#### **`cc stack push`** - Add Commit to Stack
Add the current commit to the active stack.

```bash
cc stack push [OPTIONS]

# Options:
--message <MSG>          # Override commit message for PR
--no-pr                  # Don't create PR automatically
```

**Examples:**
```bash
# Add current commit
git commit -m "Add user authentication"
cc stack push

# Push with custom PR message
cc stack push --message "Implement OAuth2 login flow"

# Push without creating PR
cc stack push --no-pr
```

#### **`cc stack pop`** - Remove Entry from Stack
Remove the top entry from the stack.

```bash
cc stack pop [OPTIONS]

# Options:
--keep-branch           # Keep the associated branch
--force                 # Skip confirmation
```

**Examples:**
```bash
# Remove top entry
cc stack pop

# Keep the branch
cc stack pop --keep-branch

# Force removal
cc stack pop --force
```

#### **`cc stack submit`** - Create Pull Request
Submit a stack entry as a pull request.

```bash
cc stack submit [ENTRY] [OPTIONS]

# Arguments:
[ENTRY]                 # Entry index (default: top entry)

# Options:
--title <TITLE>         # PR title override
--description <DESC>    # PR description
--draft                 # Create as draft PR
--reviewers <USERS>     # Comma-separated reviewer list
```

**Examples:**
```bash
# Submit top entry
cc stack submit

# Submit specific entry
cc stack submit 2

# Submit with custom details
cc stack submit --title "Add OAuth integration" --description "Implements Google OAuth2 flow"

# Create draft PR
cc stack submit --draft

# Add reviewers
cc stack submit --reviewers "alice,bob,charlie"
```

#### **`cc stack sync`** - Synchronize with Remote
Update stack with latest changes from base branch and dependencies.

```bash
cc stack sync [OPTIONS]

# Options:
--force                 # Force sync even with conflicts
--strategy <STRATEGY>   # Sync strategy (merge, rebase, cherry-pick)
```

**Examples:**
```bash
# Standard sync
cc stack sync

# Force sync with conflicts
cc stack sync --force

# Use specific strategy
cc stack sync --strategy rebase
```

#### **`cc stack rebase`** - Rebase Stack
Rebase all stack entries on latest base branch using smart force push strategy.

```bash
cc stack rebase [OPTIONS]

# Options:
--interactive          # Interactive rebase mode
--strategy <STRATEGY>  # Rebase strategy (cherry-pick, merge)
--continue            # Continue after resolving conflicts
--abort               # Abort rebase operation
```

**Smart Force Push Behavior:**
When rebasing, Cascade CLI:
1. Creates temporary versioned branches (`feature-v2`)
2. Force pushes new content to original branches (`feature`)
3. **Preserves ALL existing PRs** and review history
4. Keeps versioned branches as backup for safety

This approach follows industry standards (Graphite, Phabricator, GitHub CLI) and ensures reviewers never lose context, comments, or approval history.

**Examples:**
```bash
# Standard rebase with PR history preservation
cc stack rebase

# Interactive rebase
cc stack rebase --interactive

# Continue after conflict resolution
cc stack rebase --continue

# Abort rebase
cc stack rebase --abort
```

**What you'll see:**
```bash
$ cc stack rebase

🔄 Rebasing stack: authentication
   📋 Branch mapping:
      add-auth -> add-auth-v2      # Temporary rebase branches
      add-tests -> add-tests-v2

   🔄 Preserved pull request history:
      ✅ Force-pushed add-auth-v2 content to add-auth (preserves PR #123)
      ✅ Force-pushed add-tests-v2 content to add-tests (preserves PR #124)

   ✅ 2 commits successfully rebased
```

### **📊 Status and Information**

#### **`cc status`** - Show Status
Display comprehensive status of current repository and stacks.

```bash
cc status [OPTIONS]

# Options:
--verbose, -v           # Show detailed information
--format <FORMAT>       # Output format (table, json, yaml)
```

**Output includes:**
- Repository status
- Active stack information
- Uncommitted changes
- Pull request status
- Sync status with remotes

#### **`cc stack status`** - Stack-Specific Status
Show detailed status for current or specified stack.

```bash
cc stack status [NAME]

# Arguments:
[NAME]                  # Stack name (defaults to active stack)
```

#### **`cc stack prs`** - List Pull Requests
Show all pull requests associated with stacks.

```bash
cc stack prs [OPTIONS]

# Options:
--stack <NAME>          # Filter by stack name
--status <STATUS>       # Filter by PR status (open, merged, declined)
--format <FORMAT>       # Output format (table, json)
```

**Examples:**
```bash
# All PRs
cc stack prs

# PRs for specific stack
cc stack prs --stack feature-auth

# Only open PRs
cc stack prs --status open
```

### **🎨 Visualization**

#### **`cc viz stack`** - Stack Diagram
Generate visual representation of a stack.

```bash
cc viz stack [NAME] [OPTIONS]

# Arguments:
[NAME]                  # Stack name (defaults to active stack)

# Options:
--format <FORMAT>       # Output format (ascii, mermaid, dot, plantuml)
--output <FILE>         # Save to file
--compact              # Compact display mode
--no-colors            # Disable colored output
```

**Examples:**
```bash
# ASCII diagram in terminal
cc viz stack

# Mermaid diagram
cc viz stack --format mermaid

# Save to file
cc viz stack --format dot --output stack.dot

# Compact mode
cc viz stack --compact
```

#### **`cc viz deps`** - Dependency Graph
Show dependencies between all stacks.

```bash
cc viz deps [OPTIONS]

# Options:
--format <FORMAT>       # Output format (ascii, mermaid, dot, plantuml)
--output <FILE>         # Save to file
--compact              # Compact display mode
--no-colors            # Disable colored output
```

**Examples:**
```bash
# ASCII dependency graph
cc viz deps

# Export to Mermaid
cc viz deps --format mermaid --output deps.md

# Graphviz format for advanced visualization
cc viz deps --format dot --output deps.dot
```

### **🖥️ Interactive Tools**

#### **`cc tui`** - Terminal User Interface
Launch interactive stack browser.

```bash
cc tui
```

**Features:**
- Real-time stack visualization
- Keyboard navigation (↑/↓/Enter/q/r)
- Stack activation and switching
- Live status updates
- Error handling and recovery

**Keyboard Controls:**
- `↑/↓` - Navigate stacks
- `Enter` - Activate selected stack
- `r` - Refresh data
- `q` - Quit

### **🪝 Git Hooks**

#### **`cc hooks install`** - Install All Hooks
Install all Cascade Git hooks for workflow automation.

```bash
cc hooks install [OPTIONS]

# Options:
--force                 # Overwrite existing hooks
```

#### **`cc hooks uninstall`** - Remove All Hooks
Remove all Cascade Git hooks.

```bash
cc hooks uninstall
```

#### **`cc hooks status`** - Show Hook Status
Display installation status of all Git hooks.

```bash
cc hooks status
```

#### **`cc hooks add`** - Install Specific Hook
Install a specific Git hook.

```bash
cc hooks add <HOOK>

# Hook types:
post-commit            # Auto-add commits to active stack
pre-push              # Prevent dangerous pushes to protected branches
commit-msg            # Validate commit message format
prepare-commit-msg    # Add stack context to commit messages
```

#### **`cc hooks remove`** - Remove Specific Hook
Remove a specific Git hook.

```bash
cc hooks remove <HOOK>
```

### **⚙️ Configuration**

#### **`cc config`** - Configuration Management
Manage Cascade CLI configuration settings.

```bash
cc config <SUBCOMMAND>

# Subcommands:
list                   # Show all configuration
get <KEY>             # Get specific value
set <KEY> <VALUE>     # Set configuration value
unset <KEY>           # Remove configuration value
```

**Examples:**
```bash
# List all configuration
cc config list

# Get specific setting
cc config get bitbucket.url

# Set configuration
cc config set bitbucket.token "your-token-here"

# Remove setting
cc config unset bitbucket.project
```

### **🔧 Utility Commands**

#### **`cc doctor`** - System Diagnostics
Run comprehensive system health check.

```bash
cc doctor [OPTIONS]

# Options:
--verbose, -v           # Show detailed diagnostics
--fix                  # Attempt to fix common issues
```

#### **`cc completions`** - Shell Completions
Manage shell completion installation.

```bash
cc completions <SUBCOMMAND>

# Subcommands:
install               # Auto-install for detected shells
status               # Show installation status
generate <SHELL>     # Generate completions for specific shell
```

#### **`cc version`** - Version Information
Display version and build information.

```bash
cc version [OPTIONS]

# Options:
--verbose, -v         # Show detailed build information
```

---

## 🔄 **Workflow Patterns**

### **Feature Development Workflow**

#### **1. Start New Feature**
```bash
# Create feature stack
cc stack create feature-user-profiles --base develop --description "User profile management system"

# Start development
git checkout develop
git pull origin develop
```

#### **2. Incremental Development**
```bash
# First increment: basic profile model
git add . && git commit -m "Add user profile model"
cc stack push

# Second increment: profile endpoints
git add . && git commit -m "Add profile CRUD endpoints"
cc stack push

# Third increment: profile validation
git add . && git commit -m "Add profile data validation"
cc stack push
```

#### **3. Submit for Review**
```bash
# Submit each increment as separate PRs
cc stack submit 1  # Submit profile model
cc stack submit 2  # Submit endpoints (depends on model)
cc stack submit 3  # Submit validation (depends on endpoints)
```

#### **4. Handle Review Feedback**
```bash
# Make changes to address feedback
git add . && git commit -m "Address review feedback: improve validation"

# Update existing PR
cc stack submit 3 --title "Updated: Add profile data validation"

# Or sync if dependencies changed
cc stack sync
```

#### **5. Merge and Clean Up**
```bash
# After PRs are approved and merged
cc stack pop  # Remove merged entries
cc stack pop
cc stack pop

# Or delete completed stack
cc stack delete feature-user-profiles
```

### **Bug Fix Workflow**

#### **Quick Fix**
```bash
# Create fix stack
cc stack create fix-login-bug --base main --description "Fix login timeout issue"

# Make fix
git add . && git commit -m "Fix login timeout in OAuth flow"
cc stack push

# Submit immediately
cc stack submit --reviewers "security-team"
```

#### **Complex Fix with Investigation**
```bash
# Investigation stack
cc stack create investigate-memory-leak --base develop

# Add investigation commits
git commit -m "Add memory profiling tools"
cc stack push

git commit -m "Identify memory leak in cache layer"
cc stack push

git commit -m "Fix memory leak and add tests"
cc stack push

# Submit investigation and fix separately
cc stack submit 1 --title "Add memory profiling tools"
cc stack submit 3 --title "Fix memory leak in cache layer"
```

### **Team Collaboration Patterns**

#### **Dependent Feature Development**
```bash
# Team member A: Core infrastructure
cc stack create auth-core --base main
git commit -m "Add OAuth2 infrastructure"
cc stack push
cc stack submit

# Team member B: Dependent feature (waits for A's PR)
cc stack create user-management --base auth-core
git commit -m "Add user management using OAuth2"
cc stack push

# After A's PR is merged, B syncs
cc stack sync  # Rebase on latest main including A's changes
cc stack submit
```

#### **Parallel Development with Coordination**
```bash
# Feature A: Independent
cc stack create feature-a --base develop
# ... development work ...

# Feature B: Independent
cc stack create feature-b --base develop  
# ... development work ...

# Visualize dependencies
cc viz deps --format mermaid > team-deps.md
```

---

## 🎯 **Advanced Usage**

### **Custom Workflow Integration**

#### **CI/CD Integration**
```bash
# In CI pipeline
cc doctor --verbose           # Validate environment
cc stack status --format json # Get status for reporting
cc viz deps --format dot      # Generate dependency graphs
```

#### **Pre-commit Hook Integration**
```bash
# Install hooks for automatic workflow
cc hooks install

# Hooks will automatically:
# - Add commits to active stack
# - Validate commit messages
# - Prevent dangerous operations
```

### **Large Repository Optimization**

#### **Performance Configuration**
```bash
# Optimize for large repos
cc config set performance.cache_size 2000
cc config set performance.parallel_operations true
cc config set network.timeout 120
```

#### **Selective Stack Management**
```bash
# Work with specific stacks only
cc stack list --format name | grep feature- | xargs -I {} cc stack validate {}
```

### **Advanced Visualization**

#### **Documentation Generation**
```bash
# Generate project architecture docs
cc viz deps --format mermaid --output docs/architecture.md

# Include in markdown
echo "# Project Architecture" > docs/full-arch.md
echo "## Stack Dependencies" >> docs/full-arch.md
cc viz deps --format mermaid >> docs/full-arch.md
```

#### **Custom Formats for Tools**
```bash
# Export for external tools
cc viz stack --format dot | dot -Tpng > stack-diagram.png
cc viz deps --format plantuml | plantuml -pipe > deps.svg
```

---

## ⚙️ **Configuration**

### **Configuration File Location**
```
~/.cascade/config.toml          # User configuration
./.cascade/config.toml          # Repository configuration (overrides user)
```

### **Complete Configuration Reference**

```toml
[bitbucket]
url = "https://bitbucket.company.com"
project = "PROJECT_KEY"
repository = "repo-name"
token = "your-personal-access-token"

[git]
default_branch = "main"
auto_sync = true
conflict_strategy = "cherry-pick"

[workflow]
auto_submit = false
require_pr_template = true
default_reviewers = ["team-lead", "senior-dev"]

[ui]
colors = true
progress_bars = true
emoji = true

[performance]
cache_size = 1000
parallel_operations = true
timeout = 60

[hooks]
post_commit = true
pre_push = true
commit_msg = true
prepare_commit_msg = false
```

### **Environment Variables**
```bash
CASCADE_CONFIG_DIR="/custom/config/path"
CASCADE_LOG_LEVEL="debug"
BITBUCKET_TOKEN="token-from-env"
BITBUCKET_URL="https://bitbucket.company.com"
```

---

## 🚨 **Troubleshooting**

### **Common Issues and Solutions**

#### **"Stack not found" errors**
```bash
# List all stacks to verify names
cc stack list

# Check if in correct repository
cc status

# Re-initialize if needed
cc init --force
```

#### **Bitbucket connection issues**
```bash
# Test connection
cc doctor

# Verify token permissions
cc config get bitbucket.token

# Reconfigure if needed
cc setup --force
```

#### **Sync conflicts**
```bash
# Check conflict status
cc stack status

# Resolve manually and continue
git add .
cc stack rebase --continue

# Or abort and try different strategy
cc stack rebase --abort
cc stack sync --strategy merge
```

#### **Performance issues**
```bash
# Check repository size
du -sh .git/

# Optimize Git repository
git gc --aggressive
git prune

# Adjust cache settings
cc config set performance.cache_size 500
```

### **Debug Mode**
```bash
# Enable debug logging
export CASCADE_LOG_LEVEL=debug
cc stack push

# Check logs
tail -f ~/.cascade/logs/cascade.log
```

### **Getting Help**
```bash
# Built-in help
cc --help
cc stack --help
cc stack create --help

# System diagnostics
cc doctor --verbose

# Check configuration
cc config list
```

---

## 📞 **Support Resources**

- **[Installation Guide](./INSTALLATION.md)** - Setup and installation help
- **[Troubleshooting Guide](./TROUBLESHOOTING.md)** - Common issues and solutions
- **[Configuration Reference](./CONFIGURATION.md)** - Complete settings guide
- **[GitHub Issues](https://github.com/JAManfredi/cascade-cli/issues)** - Bug reports and feature requests
- **[Discussions](https://github.com/JAManfredi/cascade-cli/discussions)** - Community support

---

*For more detailed information on specific topics, see the linked documentation files.* 