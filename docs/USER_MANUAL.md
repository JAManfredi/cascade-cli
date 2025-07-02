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

#### **`ca init`** - Initialize Repository
Initialize Cascade CLI in a Git repository.

```bash
ca init [OPTIONS]

# Options:
--bitbucket-url <URL>     # Bitbucket Server URL
--project <PROJECT>       # Project key
--repository <REPO>       # Repository name
--force                   # Overwrite existing configuration
```

**Examples:**
```bash
# Interactive initialization
ca init

# Manual configuration
ca init --bitbucket-url https://bitbucket.company.com --project DEV --repository my-app

# Force reconfiguration
ca init --force
```

#### **`ca setup`** - Interactive Setup Wizard
Guided configuration for first-time users.

```bash
ca setup [OPTIONS]

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

#### **`ca stacks create`** - Create New Stack
Create a new stack for organizing related commits.

```bash
ca stacks create <NAME> [OPTIONS]

# Options:
--base <BRANCH>           # Base branch (default: current branch)
--description <DESC>      # Stack description
--activate               # Activate after creation (default: true)
```

**Examples:**
```bash
# Basic stack creation
ca stacks create feature-auth --base develop

# With description
ca stacks create fix-performance --base main --description "Database query optimizations"

# Create without activating
ca stacks create future-feature --base develop --no-activate
```

#### **`ca stacks list`** - List All Stacks
Display all stacks with their status and information.

```bash
ca stacks list [OPTIONS]

# Options:
--verbose, -v            # Show detailed information
--active                 # Show only active stack
--format <FORMAT>        # Output format (name, id, status)
```

**Examples:**
```bash
# Simple list
ca stacks list

# Detailed view
ca stacks list --verbose

# Only active stack
ca stacks list --active

# Custom format
ca stacks list --format status
```

#### **`ca stack`** - Display Stack Details
Show detailed information about a specific stack.

```bash
ca stack [NAME]

# Arguments:
[NAME]                   # Stack name (defaults to active stack)
```

**Output includes:**
- Stack metadata (name, description, base branch)
- All stack entries with commit details
- Pull request status and links
- Dependency information

#### **`ca switch`** - Activate Stack
Switch to a different stack, making it the active stack.

```bash
ca switch <NAME>

# Arguments:
<NAME>                   # Stack name to activate
```

**Examples:**
```bash
ca switch feature-auth
ca switch fix-bugs
```

#### **`ca stacks delete`** - Remove Stack
Delete a stack and optionally its associated branches.

```bash
ca stacks delete <NAME> [OPTIONS]

# Options:
--force                  # Skip confirmation prompt
--keep-branches         # Keep associated branches
```

**Examples:**
```bash
# With confirmation
ca stacks delete old-feature

# Force deletion
ca stacks delete temp-stack --force

# Delete but keep branches
ca stacks delete feature-x --keep-branches
```

### **🎯 Entry Editing (Modern Convenience)**

Cascade CLI provides modern convenience commands for editing specific stack entries without manual Git operations.

#### **`ca entry checkout`** - Interactive Entry Checkout
Checkout a specific stack entry for editing with intelligent tracking.

```bash
ca entry checkout [ENTRY] [OPTIONS]

# Arguments:
[ENTRY]                  # Entry number (1-based index)

# Options:
--direct                 # Skip interactive picker when using entry number
--yes, -y               # Skip confirmation prompts
```

**Examples:**
```bash
# Interactive picker (recommended)
ca entry checkout
# Shows TUI interface to select which entry to edit

# Direct checkout
ca entry checkout 1     # Checkout first entry
ca entry checkout 3 --yes  # Skip confirmation

# Direct mode (for scripting)
ca entry checkout 2 --direct --yes
```

**What it does:**
- ✅ Enters edit mode tracking for safety
- ✅ Checks out the specific commit safely
- ✅ Preserves stack state and metadata
- ✅ Shows clear guidance for next steps

#### **`ca entry status`** - Edit Mode Status
Show current edit mode status and guidance.

```bash
ca entry status [OPTIONS]

# Options:
--quiet                  # Brief status output (for scripts)
```

**Examples:**
```bash
# Detailed status
ca entry status

# Brief output
ca entry status --quiet
# Output: "active:entry-uuid" or "inactive"
```

#### **`ca entry list`** - List Entries with Edit Status
List all entries in the active stack with edit status indicators.

```bash
ca entry list [OPTIONS]

# Options:
--verbose, -v           # Show detailed information
```

**Examples:**
```bash
# Basic list with edit indicators
ca entry list

# Detailed view
ca entry list --verbose
```

**🎯 Modern Entry Editing Workflow:**
```bash
# 1. Select entry to edit
ca entry checkout         # Interactive picker

# 2. Make changes normally
git add src/auth.rs
git commit --amend -m "Add database schema (fixed column types)"

# 3. Update all dependent entries automatically
ca rebase               # Cascade handles dependencies

# 4. Verify changes
ca entry list           # Check updated status
ca stack               # See full stack state
```

**💡 Benefits over Manual Git:**
- **Safety**: Tracks edit state, prevents corruption
- **Convenience**: No need to remember commit hashes
- **Intelligence**: Interactive picker with rich information
- **Guidance**: Clear next steps and status tracking

### **📤 Stack Operations**

#### **`ca push`** - Add Commits to Stack
Add commits to the active stack. By default, pushes all unpushed commits.

```bash
ca push [OPTIONS]

# Options:
--branch <NAME>         # Custom branch name for this commit
--message <MSG>         # Commit message (if creating new commit)
--commit <HASH>         # Use specific commit instead of HEAD
--since <REF>           # Push commits since reference (e.g., HEAD~3)
--commits <HASHES>      # Push specific commits (comma-separated)
--squash <N>            # 🎉 Squash last N commits into 1 clean commit
--squash-since <REF>    # 🎉 Squash all commits since reference
```

**Default Behavior:** When no specific targeting options are provided, `ca push` pushes **all unpushed commits** since the last stack push.

**Squash Workflow Examples:**
```bash
# Make incremental commits during development
git commit -m "WIP: start feature"
git commit -m "WIP: add core logic"
git commit -m "WIP: fix bugs"
git commit -m "Final: complete feature with tests"

# 🔍 See unpushed commits and get squash suggestions
ca stack
# 🚧 Unpushed commits (4): use 'ca stacks push --squash 4' to squash them
#    1. WIP: start feature (abc123)
#    2. WIP: add core logic (def456)
#    3. WIP: fix bugs (ghi789)
#    4. Final: complete feature with tests (jkl012)
# 💡 Squash options:
#    ca stacks push --squash 4           # Squash all unpushed commits
#    ca stacks push --squash 3           # Squash last 3 commits only

# 🎉 Smart squash automatically detects "Final:" commits and creates intelligent messages
ca stacks push --squash 4
# ✅ Smart message: Complete feature with tests (automatically extracted from "Final:" commit)

# Alternative patterns that smart squash recognizes:
git commit -m "WIP: authentication work"
git commit -m "Add user authentication with OAuth"  # Uses this descriptive message
ca stacks push --squash 2  # Result: "Add user authentication with OAuth"

git commit -m "fix typo"
git commit -m "fix bug"  
git commit -m "refactor cleanup"
ca stacks push --squash 3  # Result: "Refactor cleanup" (uses last commit)
```

**Branch Naming:** Generated from final squashed commit message using Cascade CLI's branch naming rules.

**Examples:**
```bash
# Push all unpushed commits (default behavior)
git commit -m "Add user authentication"
git commit -m "Add password validation"
ca stacks push  # Pushes both commits as separate stack entries

# Push specific commit only
ca stacks push --commit abc123

# Push commits since specific reference
ca stacks push --since HEAD~3

# Push specific commits
ca stacks push --commits abc123,def456,ghi789

# Push with custom branch name
ca stacks push --branch custom-auth-branch

# Squash multiple commits before pushing
ca stacks push --squash 3  # Squashes last 3 commits into one

# Squash commits since reference
ca stacks push --squash-since HEAD~5
```

#### **`ca stacks pop`** - Remove Entry from Stack
Remove the top entry from the stack.

```bash
ca stacks pop [OPTIONS]

# Options:
--keep-branch           # Keep the associated branch
--force                 # Skip confirmation
```

**Examples:**
```bash
# Remove top entry
ca pop

# Keep the branch
ca pop --keep-branch

# Force removal
ca pop --force
```

#### **`ca submit`** - Create Pull Requests
Submit stack entries as pull requests. By default, submits all unsubmitted entries.

```bash
ca submit [ENTRY] [OPTIONS]

# Arguments:
[ENTRY]                 # Entry index (defaults to all unsubmitted entries)

# Options:
--title <TITLE>         # PR title override
--description <DESC>    # PR description
--range <RANGE>         # Submit range of entries (e.g., "1-3" or "2,4,6")
--draft                 # Create as draft PR
--reviewers <USERS>     # Comma-separated reviewer list
```

**Default Behavior:** When no specific entry is provided, `ca submit` submits **all unsubmitted entries** as separate pull requests.

**Examples:**
```bash
# Submit all unsubmitted entries (default behavior)
ca submit

# Submit specific entry
ca submit 2

# Submit range of entries
ca submit --range 1-3

# Submit specific entries 
ca submit --range 2,4,6

# Submit with custom details
ca submit --title "Add OAuth integration" --description "Implements Google OAuth2 flow"

# Create draft PRs
ca submit --draft

# Add reviewers
ca submit --reviewers "alice,bob,charlie"
```

#### **`ca sync`** - Synchronize with Remote
Update stack with latest changes from base branch and dependencies.

```bash
ca sync [OPTIONS]

# Options:
--force                 # Force sync even with conflicts
--strategy <STRATEGY>   # Sync strategy (merge, rebase, cherry-pick)
```

**Examples:**
```bash
# Standard sync
ca sync

# Force sync with conflicts
ca sync --force

# Use specific strategy
ca sync --strategy rebase
```

#### **`ca rebase`** - Rebase Stack
Rebase all stack entries on latest base branch using smart force push strategy.

```bash
ca rebase [OPTIONS]

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
ca stacks rebase

# Interactive rebase
ca stacks rebase --interactive

# Continue after conflict resolution
ca stacks rebase --continue

# Abort rebase
ca stacks rebase --abort
```

**What you'll see:**
```bash
$ ca stacks rebase

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

#### **`ca repo`** - Show Repository Overview
Display comprehensive status of current repository and stacks.

```bash
ca repo [OPTIONS]

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

#### **`ca stacks status`** - Stack-Specific Status
Show detailed status for current or specified stack.

```bash
ca stacks status [NAME]

# Arguments:
[NAME]                  # Stack name (defaults to active stack)
```

#### **`ca stacks prs`** - List Pull Requests
Show all pull requests associated with stacks.

```bash
ca stacks prs [OPTIONS]

# Options:
--stack <NAME>          # Filter by stack name
--status <STATUS>       # Filter by PR status (open, merged, declined)
--format <FORMAT>       # Output format (table, json)
```

**Examples:**
```bash
# All PRs
ca stacks prs

# PRs for specific stack
ca stacks prs --stack feature-auth

# Only open PRs
ca stacks prs --status open
```

### **🎨 Visualization**

#### **`ca viz stack`** - Stack Diagram
Generate visual representation of a stack.

```bash
ca viz stack [NAME] [OPTIONS]

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
ca viz stack

# Mermaid diagram
ca viz stack --format mermaid

# Save to file
ca viz stack --format dot --output stack.dot

# Compact mode
ca viz stack --compact
```

#### **`ca viz deps`** - Dependency Graph
Show dependencies between all stacks.

```bash
ca viz deps [OPTIONS]

# Options:
--format <FORMAT>       # Output format (ascii, mermaid, dot, plantuml)
--output <FILE>         # Save to file
--compact              # Compact display mode
--no-colors            # Disable colored output
```

**Examples:**
```bash
# ASCII dependency graph
ca viz deps

# Export to Mermaid
ca viz deps --format mermaid --output deps.md

# Graphviz format for advanced visualization
ca viz deps --format dot --output deps.dot
```

### **🖥️ Interactive Tools**

#### **`ca tui`** - Terminal User Interface
Launch interactive stack browser.

```bash
ca tui
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

#### **`ca hooks install`** - Install All Hooks
Install all Cascade Git hooks for workflow automation.

```bash
ca hooks install [OPTIONS]

# Options:
--force                 # Overwrite existing hooks
```

#### **`ca hooks uninstall`** - Remove All Hooks
Remove all Cascade Git hooks.

```bash
ca hooks uninstall
```

#### **`ca hooks status`** - Show Hook Status
Display installation status of all Git hooks.

```bash
ca hooks status
```

#### **`ca hooks add`** - Install Specific Hook
Install a specific Git hook.

```bash
ca hooks add <HOOK>

# Hook types:
post-commit            # Auto-add commits to active stack
pre-push              # Prevent dangerous pushes to protected branches
commit-msg            # Validate commit message format
prepare-commit-msg    # Add stack context to commit messages
```

#### **`ca hooks remove`** - Remove Specific Hook
Remove a specific Git hook.

```bash
ca hooks remove <HOOK>
```

### **⚙️ Configuration**

#### **`ca config`** - Configuration Management
Manage Cascade CLI configuration settings.

```bash
ca config <SUBCOMMAND>

# Subcommands:
list                   # Show all configuration
get <KEY>             # Get specific value
set <KEY> <VALUE>     # Set configuration value
unset <KEY>           # Remove configuration value
```

**Examples:**
```bash
# List all configuration
ca config list

# Get specific setting
ca config get bitbucket.url

# Set configuration
ca config set bitbucket.token "your-token-here"

# Remove setting
ca config unset bitbucket.project
```

### **🔧 Utility Commands**

#### **`ca doctor`** - System Diagnostics
Run comprehensive system health check.

```bash
ca doctor [OPTIONS]

# Options:
--verbose, -v           # Show detailed diagnostics
--fix                  # Attempt to fix common issues
```

#### **`ca completions`** - Shell Completions
Manage shell completion installation.

```bash
ca completions <SUBCOMMAND>

# Subcommands:
install               # Auto-install for detected shells
status               # Show installation status
generate <SHELL>     # Generate completions for specific shell
```

#### **`ca version`** - Version Information
Display version and build information.

```bash
ca version [OPTIONS]

# Options:
--verbose, -v         # Show detailed build information
```

---

## 🔄 **Workflow Patterns**

### **Feature Development Workflow**

#### **1. Start New Feature**
```bash
# Create feature stack
ca stacks create feature-user-profiles --base develop --description "User profile management system"

# Start development
git checkout develop
git pull origin develop
```

#### **2. Incremental Development**
```bash
# First increment: basic profile model
git add . && git commit -m "Add user profile model"
ca push

# Second increment: profile endpoints
git add . && git commit -m "Add profile CRUD endpoints"
ca push

# Third increment: profile validation
git add . && git commit -m "Add profile data validation"
ca push
```

#### **3. Submit for Review**
```bash
# Submit each increment as separate PRs
ca submit 1  # Submit profile model
ca submit 2  # Submit endpoints (depends on model)
ca submit 3  # Submit validation (depends on endpoints)
```

#### **4. Handle Review Feedback**
```bash
# Make changes to address feedback
git add . && git commit -m "Address review feedback: improve validation"

# Update existing PR
ca submit 3 --title "Updated: Add profile data validation"

# Or sync if dependencies changed
ca sync
```

#### **5. Merge and Clean Up**
```bash
# After PRs are approved and merged
ca pop  # Remove merged entries
ca pop
ca pop

# Or delete completed stack
ca stacks delete feature-user-profiles
```

### **Bug Fix Workflow**

#### **Quick Fix**
```bash
# Create fix stack
ca stacks create fix-login-bug --base main --description "Fix login timeout issue"

# Make fix
git add . && git commit -m "Fix login timeout in OAuth flow"
ca stacks push

# Submit immediately
ca stacks submit --reviewers "security-team"
```

#### **Complex Fix with Investigation**
```bash
# Investigation stack
ca stacks create investigate-memory-leak --base develop

# Add investigation commits
git commit -m "Add memory profiling tools"
ca stacks push

git commit -m "Identify memory leak in cache layer"
ca stacks push

git commit -m "Fix memory leak and add tests"
ca stacks push

# Submit investigation and fix separately
ca stacks submit 1 --title "Add memory profiling tools"
ca stacks submit 3 --title "Fix memory leak in cache layer"
```

### **Team Collaboration Patterns**

#### **Dependent Feature Development**
```bash
# Team member A: Core infrastructure
ca stacks create auth-core --base main
git commit -m "Add OAuth2 infrastructure"
ca stacks push
ca stacks submit

# Team member B: Dependent feature (waits for A's PR)
ca stacks create user-management --base auth-core
git commit -m "Add user management using OAuth2"
ca stacks push

# After A's PR is merged, B syncs
ca stacks sync  # Rebase on latest main including A's changes
ca stacks submit
```

#### **Parallel Development with Coordination**
```bash
# Feature A: Independent
ca stacks create feature-a --base develop
# ... development work ...

# Feature B: Independent
ca stacks create feature-b --base develop  
# ... development work ...

# Visualize dependencies
ca viz deps --format mermaid > team-deps.md
```

---

## 🎯 **Advanced Usage**

### **Custom Workflow Integration**

#### **CI/CD Integration**
```bash
# In CI pipeline
ca doctor --verbose           # Validate environment
ca stacks status --format json # Get status for reporting
ca viz deps --format dot      # Generate dependency graphs
```

#### **Pre-commit Hook Integration**
```bash
# Install hooks for automatic workflow
ca hooks install

# Hooks will automatically:
# - Add commits to active stack
# - Validate commit messages
# - Prevent dangerous operations
```

### **Large Repository Optimization**

#### **Performance Configuration**
```bash
# Optimize for large repos
ca config set performance.cache_size 2000
ca config set performance.parallel_operations true
ca config set network.timeout 120
```

#### **Selective Stack Management**
```bash
# Work with specific stacks only
ca stacks list --format name | grep feature- | xargs -I {} ca stacks validate {}
```

### **Advanced Visualization**

#### **Documentation Generation**
```bash
# Generate project architecture docs
ca viz deps --format mermaid --output docs/architecture.md

# Include in markdown
echo "# Project Architecture" > docs/full-arch.md
echo "## Stack Dependencies" >> docs/full-arch.md
ca viz deps --format mermaid >> docs/full-arch.md
```

#### **Custom Formats for Tools**
```bash
# Export for external tools
ca viz stack --format dot | dot -Tpng > stack-diagram.png
ca viz deps --format plantuml | plantuml -pipe > deps.svg
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
ca stacks list

# Check if in correct repository
ca repo

# Re-initialize if needed
ca init --force
```

#### **Bitbucket connection issues**
```bash
# Test connection
ca doctor

# Verify token permissions
ca config get bitbucket.token

# Reconfigure if needed
ca setup --force
```

#### **Sync conflicts**
```bash
# Check conflict status
ca stacks status

# Resolve manually and continue
git add .
ca stacks rebase --continue

# Or abort and try different strategy
ca stacks rebase --abort
ca stacks sync --strategy merge
```

#### **Performance issues**
```bash
# Check repository size
du -sh .git/

# Optimize Git repository
git gc --aggressive
git prune

# Adjust cache settings
ca config set performance.cache_size 500
```

### **Debug Mode**
```bash
# Enable debug logging
export CASCADE_LOG_LEVEL=debug
ca stacks push

# Check logs
tail -f ~/.cascade/logs/cascade.log
```

### **Getting Help**
```bash
# Built-in help
ca --help
ca stack --help
ca stacks create --help

# System diagnostics
ca doctor --verbose

# Check configuration
ca config list
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