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

#### **`csc init`** - Initialize Repository
Initialize Cascade CLI in a Git repository.

```bash
csc init [OPTIONS]

# Options:
--bitbucket-url <URL>     # Bitbucket Server URL
--project <PROJECT>       # Project key
--repository <REPO>       # Repository name
--force                   # Overwrite existing configuration
```

**Examples:**
```bash
# Interactive initialization
csc init

# Manual configuration
csc init --bitbucket-url https://bitbucket.company.com --project DEV --repository my-app

# Force reconfiguration
csc init --force
```

#### **`csc setup`** - Interactive Setup Wizard
Guided configuration for first-time users.

```bash
csc setup [OPTIONS]

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

#### **`csc stacks create`** - Create New Stack
Create a new stack for organizing related commits.

```bash
csc stacks create <NAME> [OPTIONS]

# Options:
--base <BRANCH>           # Base branch (default: current branch)
--description <DESC>      # Stack description
--activate               # Activate after creation (default: true)
```

**Examples:**
```bash
# Basic stack creation
csc stacks create feature-auth --base develop

# With description
csc stacks create fix-performance --base main --description "Database query optimizations"

# Create without activating
csc stacks create future-feature --base develop --no-activate
```

#### **`csc stacks list`** - List All Stacks
Display all stacks with their status and information.

```bash
csc stacks list [OPTIONS]

# Options:
--verbose, -v            # Show detailed information
--active                 # Show only active stack
--format <FORMAT>        # Output format (name, id, status)
```

**Examples:**
```bash
# Simple list
csc stacks list

# Detailed view
csc stacks list --verbose

# Only active stack
csc stacks list --active

# Custom format
csc stacks list --format status
```

#### **`csc stack`** - Display Stack Details
Show detailed information about a specific stack.

```bash
csc stack [NAME]

# Arguments:
[NAME]                   # Stack name (defaults to active stack)
```

**Output includes:**
- Stack metadata (name, description, base branch)
- All stack entries with commit details
- Pull request status and links
- Dependency information

#### **`csc stacks switch`** - Activate Stack
Switch to a different stack, making it the active stack.

```bash
csc stacks switch <NAME>

# Arguments:
<NAME>                   # Stack name to activate
```

**Examples:**
```bash
csc stacks switch feature-auth
csc stacks switch fix-bugs
```

#### **`csc stacks delete`** - Remove Stack
Delete a stack and optionally its associated branches.

```bash
csc stacks delete <NAME> [OPTIONS]

# Options:
--force                  # Skip confirmation prompt
--keep-branches         # Keep associated branches
```

**Examples:**
```bash
# With confirmation
csc stacks delete old-feature

# Force deletion
csc stacks delete temp-stack --force

# Delete but keep branches
csc stacks delete feature-x --keep-branches
```

### **📤 Stack Operations**

#### **`csc stacks push`** - Add Commits to Stack
Add commits to the active stack. By default, pushes all unpushed commits.

```bash
csc stacks push [OPTIONS]

# Options:
--branch <NAME>         # Custom branch name for this commit
--message <MSG>         # Commit message (if creating new commit)
--commit <HASH>         # Use specific commit instead of HEAD
--since <REF>           # Push commits since reference (e.g., HEAD~3)
--commits <HASHES>      # Push specific commits (comma-separated)
--squash <N>            # 🎉 Squash last N commits into 1 clean commit
--squash-since <REF>    # 🎉 Squash all commits since reference
```

**Default Behavior:** When no specific targeting options are provided, `csc stacks push` pushes **all unpushed commits** since the last stack push.

**Squash Workflow Examples:**
```bash
# Make incremental commits during development
git commit -m "WIP: start feature"
git commit -m "WIP: add core logic"
git commit -m "WIP: fix bugs"
git commit -m "Final: complete feature with tests"

# 🔍 See unpushed commits and get squash suggestions
csc stack
# 🚧 Unpushed commits (4): use 'csc stacks push --squash 4' to squash them
#    1. WIP: start feature (abc123)
#    2. WIP: add core logic (def456)
#    3. WIP: fix bugs (ghi789)
#    4. Final: complete feature with tests (jkl012)
# 💡 Squash options:
#    csc stacks push --squash 4           # Squash all unpushed commits
#    csc stacks push --squash 3           # Squash last 3 commits only

# 🎉 Smart squash automatically detects "Final:" commits and creates intelligent messages
csc stacks push --squash 4
# ✅ Smart message: Complete feature with tests (automatically extracted from "Final:" commit)

# Alternative patterns that smart squash recognizes:
git commit -m "WIP: authentication work"
git commit -m "Add user authentication with OAuth"  # Uses this descriptive message
csc stacks push --squash 2  # Result: "Add user authentication with OAuth"

git commit -m "fix typo"
git commit -m "fix bug"  
git commit -m "refactor cleanup"
csc stacks push --squash 3  # Result: "Refactor cleanup" (uses last commit)
```

**Branch Naming:** Generated from final squashed commit message using Cascade CLI's branch naming rules.

**Examples:**
```bash
# Push all unpushed commits (default behavior)
git commit -m "Add user authentication"
git commit -m "Add password validation"
csc stacks push  # Pushes both commits as separate stack entries

# Push specific commit only
csc stacks push --commit abc123

# Push commits since specific reference
csc stacks push --since HEAD~3

# Push specific commits
csc stacks push --commits abc123,def456,ghi789

# Push with custom branch name
csc stacks push --branch custom-auth-branch

# Squash multiple commits before pushing
csc stacks push --squash 3  # Squashes last 3 commits into one

# Squash commits since reference
csc stacks push --squash-since HEAD~5
```

#### **`csc stacks pop`** - Remove Entry from Stack
Remove the top entry from the stack.

```bash
csc stacks pop [OPTIONS]

# Options:
--keep-branch           # Keep the associated branch
--force                 # Skip confirmation
```

**Examples:**
```bash
# Remove top entry
csc stacks pop

# Keep the branch
csc stacks pop --keep-branch

# Force removal
csc stacks pop --force
```

#### **`csc stacks submit`** - Create Pull Requests
Submit stack entries as pull requests. By default, submits all unsubmitted entries.

```bash
csc stacks submit [ENTRY] [OPTIONS]

# Arguments:
[ENTRY]                 # Entry index (defaults to all unsubmitted entries)

# Options:
--title <TITLE>         # PR title override
--description <DESC>    # PR description
--range <RANGE>         # Submit range of entries (e.g., "1-3" or "2,4,6")
--draft                 # Create as draft PR
--reviewers <USERS>     # Comma-separated reviewer list
```

**Default Behavior:** When no specific entry is provided, `csc stacks submit` submits **all unsubmitted entries** as separate pull requests.

**Examples:**
```bash
# Submit all unsubmitted entries (default behavior)
csc stacks submit

# Submit specific entry
csc stacks submit 2

# Submit range of entries
csc stacks submit --range 1-3

# Submit specific entries 
csc stacks submit --range 2,4,6

# Submit with custom details
csc stacks submit --title "Add OAuth integration" --description "Implements Google OAuth2 flow"

# Create draft PRs
csc stacks submit --draft

# Add reviewers
csc stacks submit --reviewers "alice,bob,charlie"
```

#### **`csc stacks sync`** - Synchronize with Remote
Update stack with latest changes from base branch and dependencies.

```bash
csc stacks sync [OPTIONS]

# Options:
--force                 # Force sync even with conflicts
--strategy <STRATEGY>   # Sync strategy (merge, rebase, cherry-pick)
```

**Examples:**
```bash
# Standard sync
csc stacks sync

# Force sync with conflicts
csc stacks sync --force

# Use specific strategy
csc stacks sync --strategy rebase
```

#### **`csc stacks rebase`** - Rebase Stack
Rebase all stack entries on latest base branch using smart force push strategy.

```bash
csc stacks rebase [OPTIONS]

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
csc stacks rebase

# Interactive rebase
csc stacks rebase --interactive

# Continue after conflict resolution
csc stacks rebase --continue

# Abort rebase
csc stacks rebase --abort
```

**What you'll see:**
```bash
$ csc stacks rebase

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

#### **`csc repo`** - Show Repository Overview
Display comprehensive status of current repository and stacks.

```bash
csc repo [OPTIONS]

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

#### **`csc stacks status`** - Stack-Specific Status
Show detailed status for current or specified stack.

```bash
csc stacks status [NAME]

# Arguments:
[NAME]                  # Stack name (defaults to active stack)
```

#### **`csc stacks prs`** - List Pull Requests
Show all pull requests associated with stacks.

```bash
csc stacks prs [OPTIONS]

# Options:
--stack <NAME>          # Filter by stack name
--status <STATUS>       # Filter by PR status (open, merged, declined)
--format <FORMAT>       # Output format (table, json)
```

**Examples:**
```bash
# All PRs
csc stacks prs

# PRs for specific stack
csc stacks prs --stack feature-auth

# Only open PRs
csc stacks prs --status open
```

### **🎨 Visualization**

#### **`csc viz stack`** - Stack Diagram
Generate visual representation of a stack.

```bash
csc viz stack [NAME] [OPTIONS]

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
csc viz stack

# Mermaid diagram
csc viz stack --format mermaid

# Save to file
csc viz stack --format dot --output stack.dot

# Compact mode
csc viz stack --compact
```

#### **`csc viz deps`** - Dependency Graph
Show dependencies between all stacks.

```bash
csc viz deps [OPTIONS]

# Options:
--format <FORMAT>       # Output format (ascii, mermaid, dot, plantuml)
--output <FILE>         # Save to file
--compact              # Compact display mode
--no-colors            # Disable colored output
```

**Examples:**
```bash
# ASCII dependency graph
csc viz deps

# Export to Mermaid
csc viz deps --format mermaid --output deps.md

# Graphviz format for advanced visualization
csc viz deps --format dot --output deps.dot
```

### **🖥️ Interactive Tools**

#### **`csc tui`** - Terminal User Interface
Launch interactive stack browser.

```bash
csc tui
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

#### **`csc hooks install`** - Install All Hooks
Install all Cascade Git hooks for workflow automation.

```bash
csc hooks install [OPTIONS]

# Options:
--force                 # Overwrite existing hooks
```

#### **`csc hooks uninstall`** - Remove All Hooks
Remove all Cascade Git hooks.

```bash
csc hooks uninstall
```

#### **`csc hooks status`** - Show Hook Status
Display installation status of all Git hooks.

```bash
csc hooks status
```

#### **`csc hooks add`** - Install Specific Hook
Install a specific Git hook.

```bash
csc hooks add <HOOK>

# Hook types:
post-commit            # Auto-add commits to active stack
pre-push              # Prevent dangerous pushes to protected branches
commit-msg            # Validate commit message format
prepare-commit-msg    # Add stack context to commit messages
```

#### **`csc hooks remove`** - Remove Specific Hook
Remove a specific Git hook.

```bash
csc hooks remove <HOOK>
```

### **⚙️ Configuration**

#### **`csc config`** - Configuration Management
Manage Cascade CLI configuration settings.

```bash
csc config <SUBCOMMAND>

# Subcommands:
list                   # Show all configuration
get <KEY>             # Get specific value
set <KEY> <VALUE>     # Set configuration value
unset <KEY>           # Remove configuration value
```

**Examples:**
```bash
# List all configuration
csc config list

# Get specific setting
csc config get bitbucket.url

# Set configuration
csc config set bitbucket.token "your-token-here"

# Remove setting
csc config unset bitbucket.project
```

### **🔧 Utility Commands**

#### **`csc doctor`** - System Diagnostics
Run comprehensive system health check.

```bash
csc doctor [OPTIONS]

# Options:
--verbose, -v           # Show detailed diagnostics
--fix                  # Attempt to fix common issues
```

#### **`csc completions`** - Shell Completions
Manage shell completion installation.

```bash
csc completions <SUBCOMMAND>

# Subcommands:
install               # Auto-install for detected shells
status               # Show installation status
generate <SHELL>     # Generate completions for specific shell
```

#### **`csc version`** - Version Information
Display version and build information.

```bash
csc version [OPTIONS]

# Options:
--verbose, -v         # Show detailed build information
```

---

## 🔄 **Workflow Patterns**

### **Feature Development Workflow**

#### **1. Start New Feature**
```bash
# Create feature stack
csc stacks create feature-user-profiles --base develop --description "User profile management system"

# Start development
git checkout develop
git pull origin develop
```

#### **2. Incremental Development**
```bash
# First increment: basic profile model
git add . && git commit -m "Add user profile model"
csc stacks push

# Second increment: profile endpoints
git add . && git commit -m "Add profile CRUD endpoints"
csc stacks push

# Third increment: profile validation
git add . && git commit -m "Add profile data validation"
csc stacks push
```

#### **3. Submit for Review**
```bash
# Submit each increment as separate PRs
csc stacks submit 1  # Submit profile model
csc stacks submit 2  # Submit endpoints (depends on model)
csc stacks submit 3  # Submit validation (depends on endpoints)
```

#### **4. Handle Review Feedback**
```bash
# Make changes to address feedback
git add . && git commit -m "Address review feedback: improve validation"

# Update existing PR
csc stacks submit 3 --title "Updated: Add profile data validation"

# Or sync if dependencies changed
csc stacks sync
```

#### **5. Merge and Clean Up**
```bash
# After PRs are approved and merged
csc stacks pop  # Remove merged entries
csc stacks pop
csc stacks pop

# Or delete completed stack
csc stacks delete feature-user-profiles
```

### **Bug Fix Workflow**

#### **Quick Fix**
```bash
# Create fix stack
csc stacks create fix-login-bug --base main --description "Fix login timeout issue"

# Make fix
git add . && git commit -m "Fix login timeout in OAuth flow"
csc stacks push

# Submit immediately
csc stacks submit --reviewers "security-team"
```

#### **Complex Fix with Investigation**
```bash
# Investigation stack
csc stacks create investigate-memory-leak --base develop

# Add investigation commits
git commit -m "Add memory profiling tools"
csc stacks push

git commit -m "Identify memory leak in cache layer"
csc stacks push

git commit -m "Fix memory leak and add tests"
csc stacks push

# Submit investigation and fix separately
csc stacks submit 1 --title "Add memory profiling tools"
csc stacks submit 3 --title "Fix memory leak in cache layer"
```

### **Team Collaboration Patterns**

#### **Dependent Feature Development**
```bash
# Team member A: Core infrastructure
csc stacks create auth-core --base main
git commit -m "Add OAuth2 infrastructure"
csc stacks push
csc stacks submit

# Team member B: Dependent feature (waits for A's PR)
csc stacks create user-management --base auth-core
git commit -m "Add user management using OAuth2"
csc stacks push

# After A's PR is merged, B syncs
csc stacks sync  # Rebase on latest main including A's changes
csc stacks submit
```

#### **Parallel Development with Coordination**
```bash
# Feature A: Independent
csc stacks create feature-a --base develop
# ... development work ...

# Feature B: Independent
csc stacks create feature-b --base develop  
# ... development work ...

# Visualize dependencies
csc viz deps --format mermaid > team-deps.md
```

---

## 🎯 **Advanced Usage**

### **Custom Workflow Integration**

#### **CI/CD Integration**
```bash
# In CI pipeline
csc doctor --verbose           # Validate environment
csc stacks status --format json # Get status for reporting
csc viz deps --format dot      # Generate dependency graphs
```

#### **Pre-commit Hook Integration**
```bash
# Install hooks for automatic workflow
csc hooks install

# Hooks will automatically:
# - Add commits to active stack
# - Validate commit messages
# - Prevent dangerous operations
```

### **Large Repository Optimization**

#### **Performance Configuration**
```bash
# Optimize for large repos
csc config set performance.cache_size 2000
csc config set performance.parallel_operations true
csc config set network.timeout 120
```

#### **Selective Stack Management**
```bash
# Work with specific stacks only
csc stacks list --format name | grep feature- | xargs -I {} csc stacks validate {}
```

### **Advanced Visualization**

#### **Documentation Generation**
```bash
# Generate project architecture docs
csc viz deps --format mermaid --output docs/architecture.md

# Include in markdown
echo "# Project Architecture" > docs/full-arch.md
echo "## Stack Dependencies" >> docs/full-arch.md
csc viz deps --format mermaid >> docs/full-arch.md
```

#### **Custom Formats for Tools**
```bash
# Export for external tools
csc viz stack --format dot | dot -Tpng > stack-diagram.png
csc viz deps --format plantuml | plantuml -pipe > deps.svg
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
csc stacks list

# Check if in correct repository
csc repo

# Re-initialize if needed
csc init --force
```

#### **Bitbucket connection issues**
```bash
# Test connection
csc doctor

# Verify token permissions
csc config get bitbucket.token

# Reconfigure if needed
csc setup --force
```

#### **Sync conflicts**
```bash
# Check conflict status
csc stacks status

# Resolve manually and continue
git add .
csc stacks rebase --continue

# Or abort and try different strategy
csc stacks rebase --abort
csc stacks sync --strategy merge
```

#### **Performance issues**
```bash
# Check repository size
du -sh .git/

# Optimize Git repository
git gc --aggressive
git prune

# Adjust cache settings
csc config set performance.cache_size 500
```

### **Debug Mode**
```bash
# Enable debug logging
export CASCADE_LOG_LEVEL=debug
csc stacks push

# Check logs
tail -f ~/.cascade/logs/cascade.log
```

### **Getting Help**
```bash
# Built-in help
csc --help
csc stack --help
csc stacks create --help

# System diagnostics
csc doctor --verbose

# Check configuration
csc config list
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