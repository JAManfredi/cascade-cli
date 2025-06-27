use crate::errors::{CascadeError, Result};
use crate::config::{CascadeConfig, Settings};
use std::env;
use std::path::{Path, PathBuf};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

/// Git repository type detection
#[derive(Debug, Clone, PartialEq)]
pub enum RepositoryType {
    Bitbucket,
    GitHub,
    GitLab,
    AzureDevOps,
    Unknown,
}

/// Branch type classification
#[derive(Debug, Clone, PartialEq)]
pub enum BranchType {
    Main,      // main, master, develop
    Feature,   // feature branches
    Unknown,
}

/// Installation options for smart hook activation
#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub check_prerequisites: bool,
    pub feature_branches_only: bool,
    pub confirm: bool,
    pub force: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            check_prerequisites: true,
            feature_branches_only: true,
            confirm: true,
            force: false,
        }
    }
}

/// Git hooks integration for Cascade CLI
pub struct HooksManager {
    repo_path: PathBuf,
    hooks_dir: PathBuf,
}

/// Available Git hooks that Cascade can install
#[derive(Debug, Clone)]
pub enum HookType {
    /// Validates commits are added to stacks
    PostCommit,
    /// Prevents force pushes and validates stack state
    PrePush,
    /// Validates commit messages follow conventions
    CommitMsg,
    /// Prepares commit message with stack context
    PrepareCommitMsg,
}

impl HookType {
    fn filename(&self) -> &'static str {
        match self {
            HookType::PostCommit => "post-commit",
            HookType::PrePush => "pre-push",
            HookType::CommitMsg => "commit-msg",
            HookType::PrepareCommitMsg => "prepare-commit-msg",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            HookType::PostCommit => "Auto-add new commits to active stack",
            HookType::PrePush => "Prevent force pushes and validate stack state",
            HookType::CommitMsg => "Validate commit message format",
            HookType::PrepareCommitMsg => "Add stack context to commit messages",
        }
    }
}

impl HooksManager {
    pub fn new(repo_path: &Path) -> Result<Self> {
        let hooks_dir = repo_path.join(".git").join("hooks");
        
        if !hooks_dir.exists() {
            return Err(CascadeError::config("Git hooks directory not found. Is this a Git repository?".to_string()));
        }

        Ok(Self {
            repo_path: repo_path.to_path_buf(),
            hooks_dir,
        })
    }

    /// Install all recommended Cascade hooks
    pub fn install_all(&self) -> Result<()> {
        self.install_with_options(&InstallOptions::default())
    }

    /// Install hooks with smart validation options
    pub fn install_with_options(&self, options: &InstallOptions) -> Result<()> {
        if options.check_prerequisites && !options.force {
            self.validate_prerequisites()?;
        }

        if options.feature_branches_only && !options.force {
            self.validate_branch_suitability()?;
        }

        if options.confirm && !options.force {
            self.confirm_installation()?;
        }

        println!("🪝 Installing Cascade Git hooks...");
        
        let hooks = vec![
            HookType::PostCommit,
            HookType::PrePush,
            HookType::CommitMsg,
            HookType::PrepareCommitMsg,
        ];

        for hook in hooks {
            self.install_hook(&hook)?;
        }

        println!("✅ All Cascade hooks installed successfully!");
        println!("\n💡 Hooks installed:");
        self.list_installed_hooks()?;
        
        Ok(())
    }

    /// Install a specific hook
    pub fn install_hook(&self, hook_type: &HookType) -> Result<()> {
        let hook_path = self.hooks_dir.join(hook_type.filename());
        let hook_content = self.generate_hook_script(hook_type)?;

        // Backup existing hook if it exists
        if hook_path.exists() {
            let backup_path = hook_path.with_extension("cascade-backup");
            fs::copy(&hook_path, &backup_path)
                .map_err(|e| CascadeError::config(format!("Failed to backup existing hook: {}", e)))?;
            println!("📦 Backed up existing {} hook", hook_type.filename());
        }

        // Write new hook
        fs::write(&hook_path, hook_content)
            .map_err(|e| CascadeError::config(format!("Failed to write hook file: {}", e)))?;

        // Make executable
        let mut perms = fs::metadata(&hook_path)
            .map_err(|e| CascadeError::config(format!("Failed to get hook file metadata: {}", e)))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)
            .map_err(|e| CascadeError::config(format!("Failed to make hook executable: {}", e)))?;

        println!("✅ Installed {} hook", hook_type.filename());
        Ok(())
    }

    /// Remove all Cascade hooks
    pub fn uninstall_all(&self) -> Result<()> {
        println!("🗑️ Removing Cascade Git hooks...");
        
        let hooks = vec![
            HookType::PostCommit,
            HookType::PrePush,
            HookType::CommitMsg,
            HookType::PrepareCommitMsg,
        ];

        for hook in hooks {
            self.uninstall_hook(&hook)?;
        }

        println!("✅ All Cascade hooks removed!");
        Ok(())
    }

    /// Remove a specific hook
    pub fn uninstall_hook(&self, hook_type: &HookType) -> Result<()> {
        let hook_path = self.hooks_dir.join(hook_type.filename());
        
        if hook_path.exists() {
            // Check if it's a Cascade hook
            let content = fs::read_to_string(&hook_path)
                .map_err(|e| CascadeError::config(format!("Failed to read hook file: {}", e)))?;
            
            if content.contains("# Cascade CLI Hook") {
                fs::remove_file(&hook_path)
                    .map_err(|e| CascadeError::config(format!("Failed to remove hook file: {}", e)))?;
                
                // Restore backup if it exists
                let backup_path = hook_path.with_extension("cascade-backup");
                if backup_path.exists() {
                    fs::rename(&backup_path, &hook_path)
                        .map_err(|e| CascadeError::config(format!("Failed to restore backup: {}", e)))?;
                    println!("📦 Restored original {} hook", hook_type.filename());
                } else {
                    println!("🗑️ Removed {} hook", hook_type.filename());
                }
            } else {
                println!("⚠️ {} hook exists but is not a Cascade hook, skipping", hook_type.filename());
            }
        } else {
            println!("ℹ️ {} hook not found", hook_type.filename());
        }

        Ok(())
    }

    /// List all installed hooks and their status
    pub fn list_installed_hooks(&self) -> Result<()> {
        let hooks = vec![
            HookType::PostCommit,
            HookType::PrePush,
            HookType::CommitMsg,
            HookType::PrepareCommitMsg,
        ];

        println!("\n📋 Git Hooks Status:");
        println!("┌─────────────────────┬──────────┬─────────────────────────────────┐");
        println!("│ Hook                │ Status   │ Description                     │");
        println!("├─────────────────────┼──────────┼─────────────────────────────────┤");

        for hook in hooks {
            let hook_path = self.hooks_dir.join(hook.filename());
            let status = if hook_path.exists() {
                let content = fs::read_to_string(&hook_path).unwrap_or_default();
                if content.contains("# Cascade CLI Hook") {
                    "✅ Cascade"
                } else {
                    "⚠️ Custom "
                }
            } else {
                "❌ Missing"
            };

            println!("│ {:19} │ {:8} │ {:31} │", 
                hook.filename(), 
                status, 
                hook.description()
            );
        }
        println!("└─────────────────────┴──────────┴─────────────────────────────────┘");

        Ok(())
    }

    /// Generate hook script content
    fn generate_hook_script(&self, hook_type: &HookType) -> Result<String> {
        let cascade_cli = env::current_exe()
            .map_err(|e| CascadeError::config(format!("Failed to get current executable path: {}", e)))?
            .to_string_lossy()
            .to_string();

        let script = match hook_type {
            HookType::PostCommit => self.generate_post_commit_hook(&cascade_cli),
            HookType::PrePush => self.generate_pre_push_hook(&cascade_cli),
            HookType::CommitMsg => self.generate_commit_msg_hook(&cascade_cli),
            HookType::PrepareCommitMsg => self.generate_prepare_commit_msg_hook(&cascade_cli),
        };

        Ok(script)
    }

    fn generate_post_commit_hook(&self, cascade_cli: &str) -> String {
        format!(r#"#!/bin/sh
# Cascade CLI Hook - Post Commit
# Automatically adds new commits to the active stack

set -e

# Get the commit hash
COMMIT_HASH=$(git rev-parse HEAD)
COMMIT_MSG=$(git log --format=%s -n 1 HEAD)
BRANCH_NAME=$(git branch --show-current)

# Check if Cascade is initialized
if [ ! -d ".cascade" ]; then
    echo "ℹ️ Cascade not initialized, skipping stack management"
    exit 0
fi

# Check if there's an active stack
if ! "{cascade_cli}" stack list --active > /dev/null 2>&1; then
    echo "ℹ️ No active stack found, commit will not be added to any stack"
    echo "💡 Use 'cc stack create' to create a stack or 'cc stack activate' to set one active"
    exit 0
fi

# Add commit to active stack
echo "🪝 Adding commit to active stack..."
if "{cascade_cli}" stack push --commit "$COMMIT_HASH" --branch "$BRANCH_NAME" --message "$COMMIT_MSG"; then
    echo "✅ Commit added to stack successfully"
else
    echo "⚠️ Failed to add commit to stack"
    echo "💡 You can manually add it with: cc stack push --commit $COMMIT_HASH"
fi
"#)
    }

    fn generate_pre_push_hook(&self, cascade_cli: &str) -> String {
        format!(r#"#!/bin/sh
# Cascade CLI Hook - Pre Push
# Prevents force pushes and validates stack state

set -e

# Check for force push
if echo "$*" | grep -q -- "--force\|--force-with-lease\|-f"; then
    echo "❌ Force push detected!"
    echo "🌊 Cascade CLI uses stacked diffs - force pushes can break the stack integrity"
    echo ""
    echo "💡 Instead of force pushing, try:"
    echo "   • cc stack sync    - Sync with remote changes"
    echo "   • cc stack rebase  - Rebase stack on latest base"
    echo "   • cc stack submit  - Submit entries for review"
    echo ""
    echo "🚨 If you really need to force push, run:"
    echo "   git push --force-with-lease [remote] [branch]"
    echo "   (But consider if this will affect other stack entries)"
    exit 1
fi

# Check if Cascade is initialized
if [ ! -d ".cascade" ]; then
    echo "ℹ️ Cascade not initialized, allowing push"
    exit 0
fi

# Validate stack state
echo "🪝 Validating stack state before push..."
if "{cascade_cli}" stack validate; then
    echo "✅ Stack validation passed"
else
    echo "❌ Stack validation failed"
    echo "💡 Fix validation errors before pushing:"
    echo "   • cc doctor           - Check overall health"
    echo "   • cc stack status     - Check stack status"
    echo "   • cc stack sync       - Sync with remote"
    exit 1
fi

echo "✅ Pre-push validation complete"
"#)
    }

    fn generate_commit_msg_hook(&self, cascade_cli: &str) -> String {
        format!(r#"#!/bin/sh
# Cascade CLI Hook - Commit Message
# Validates commit message format

set -e

COMMIT_MSG_FILE="$1"
COMMIT_MSG=$(cat "$COMMIT_MSG_FILE")

# Skip validation for merge commits, fixup commits, etc.
if echo "$COMMIT_MSG" | grep -E "^(Merge|Revert|fixup!|squash!)" > /dev/null; then
    exit 0
fi

# Check if Cascade is initialized
if [ ! -d ".cascade" ]; then
    exit 0
fi

# Basic commit message validation
if [ ${{#COMMIT_MSG}} -lt 10 ]; then
    echo "❌ Commit message too short (minimum 10 characters)"
    echo "💡 Write a descriptive commit message for better stack management"
    exit 1
fi

if [ ${{#COMMIT_MSG}} -gt 72 ]; then
    echo "⚠️ Warning: Commit message longer than 72 characters"
    echo "💡 Consider keeping the first line short for better readability"
fi

# Check for conventional commit format (optional)
if ! echo "$COMMIT_MSG" | grep -E "^(feat|fix|docs|style|refactor|test|chore|perf|ci|build)(\(.+\))?: .+" > /dev/null; then
    echo "💡 Consider using conventional commit format:"
    echo "   feat: add new feature"
    echo "   fix: resolve bug"
    echo "   docs: update documentation"
    echo "   etc."
fi

echo "✅ Commit message validation passed"
"#)
    }

    fn generate_prepare_commit_msg_hook(&self, cascade_cli: &str) -> String {
        format!(r#"#!/bin/sh
# Cascade CLI Hook - Prepare Commit Message
# Adds stack context to commit messages

set -e

COMMIT_MSG_FILE="$1"
COMMIT_SOURCE="$2"
COMMIT_SHA="$3"

# Only modify message if it's a regular commit (not merge, template, etc.)
if [ "$COMMIT_SOURCE" != "" ] && [ "$COMMIT_SOURCE" != "message" ]; then
    exit 0
fi

# Check if Cascade is initialized
if [ ! -d ".cascade" ]; then
    exit 0
fi

# Get active stack info
ACTIVE_STACK=$("{cascade_cli}" stack list --active --format=name 2>/dev/null || echo "")

if [ -n "$ACTIVE_STACK" ]; then
    # Get current commit message
    CURRENT_MSG=$(cat "$COMMIT_MSG_FILE")
    
    # Skip if message already has stack context
    if echo "$CURRENT_MSG" | grep -q "\[stack:"; then
        exit 0
    fi
    
    # Add stack context to commit message
    echo "
# Stack: $ACTIVE_STACK
# This commit will be added to the active stack automatically.
# Use 'cc stack status' to see the current stack state.
$CURRENT_MSG" > "$COMMIT_MSG_FILE"
fi
"#)
    }

    /// Detect repository type from remote URLs
    pub fn detect_repository_type(&self) -> Result<RepositoryType> {
        let output = Command::new("git")
            .args(&["remote", "get-url", "origin"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to get remote URL: {}", e)))?;

        if !output.status.success() {
            return Ok(RepositoryType::Unknown);
        }

        let remote_url = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();

        if remote_url.contains("github.com") {
            Ok(RepositoryType::GitHub)
        } else if remote_url.contains("gitlab.com") || remote_url.contains("gitlab") {
            Ok(RepositoryType::GitLab)
        } else if remote_url.contains("dev.azure.com") || remote_url.contains("visualstudio.com") {
            Ok(RepositoryType::AzureDevOps)
        } else if remote_url.contains("bitbucket") {
            Ok(RepositoryType::Bitbucket)
        } else {
            Ok(RepositoryType::Unknown)
        }
    }

    /// Detect current branch type
    pub fn detect_branch_type(&self) -> Result<BranchType> {
        let output = Command::new("git")
            .args(&["branch", "--show-current"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to get current branch: {}", e)))?;

        if !output.status.success() {
            return Ok(BranchType::Unknown);
        }

        let branch_name = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();

        if branch_name == "main" || branch_name == "master" || branch_name == "develop" {
            Ok(BranchType::Main)
        } else if !branch_name.is_empty() {
            Ok(BranchType::Feature)
        } else {
            Ok(BranchType::Unknown)
        }
    }

    /// Validate prerequisites for hook installation
    pub fn validate_prerequisites(&self) -> Result<()> {
        println!("🔍 Checking prerequisites for Cascade hooks...");

        // 1. Check repository type
        let repo_type = self.detect_repository_type()?;
        match repo_type {
            RepositoryType::Bitbucket => {
                println!("✅ Bitbucket repository detected");
            }
            RepositoryType::GitHub => {
                return Err(CascadeError::config(
                    "🚫 GitHub repository detected!\n\n\
                    Cascade CLI is designed for Bitbucket Server repositories.\n\
                    GitHub has its own stacked diff solutions:\n\
                    • GitHub CLI: https://cli.github.com/\n\
                    • git-branchless: https://github.com/arxanas/git-branchless\n\
                    • Graphite: https://graphite.dev/\n\n\
                    Use --force to install anyway (not recommended).".to_string()
                ));
            }
            RepositoryType::GitLab => {
                return Err(CascadeError::config(
                    "🚫 GitLab repository detected!\n\n\
                    Cascade CLI is designed for Bitbucket Server repositories.\n\
                    GitLab has its own merge request workflows.\n\n\
                    Use --force to install anyway (not recommended).".to_string()
                ));
            }
            RepositoryType::AzureDevOps => {
                return Err(CascadeError::config(
                    "🚫 Azure DevOps repository detected!\n\n\
                    Cascade CLI is designed for Bitbucket Server repositories.\n\
                    Azure DevOps has its own pull request workflows.\n\n\
                    Use --force to install anyway (not recommended).".to_string()
                ));
            }
            RepositoryType::Unknown => {
                println!("⚠️ Unknown repository type - proceeding with caution");
            }
        }

        // 2. Check Cascade configuration
        let config_path = self.repo_path.join(".cascade").join("config.json");
        if !config_path.exists() {
            return Err(CascadeError::config(
                "🚫 Cascade not initialized!\n\n\
                Please run 'cc init' or 'cc setup' first to configure Cascade CLI.\n\
                Hooks require proper Bitbucket Server configuration.\n\n\
                Use --force to install anyway (not recommended).".to_string()
            ));
        }

        // 3. Validate Bitbucket configuration
        let config = Settings::load_from_file(&config_path)?;

        if config.bitbucket.url == "https://bitbucket.example.com" || 
           config.bitbucket.url.contains("example.com") {
            return Err(CascadeError::config(
                "🚫 Invalid Bitbucket configuration!\n\n\
                Your Bitbucket URL appears to be a placeholder.\n\
                Please run 'cc setup' to configure a real Bitbucket Server.\n\n\
                Use --force to install anyway (not recommended).".to_string()
            ));
        }

        if config.bitbucket.project == "PROJECT" || config.bitbucket.repo == "repo" {
            return Err(CascadeError::config(
                "🚫 Incomplete Bitbucket configuration!\n\n\
                Your project/repository settings appear to be placeholders.\n\
                Please run 'cc setup' to complete configuration.\n\n\
                Use --force to install anyway (not recommended).".to_string()
            ));
        }

        println!("✅ Prerequisites validation passed");
        Ok(())
    }

    /// Validate branch suitability for hooks
    pub fn validate_branch_suitability(&self) -> Result<()> {
        let branch_type = self.detect_branch_type()?;
        
        match branch_type {
            BranchType::Main => {
                return Err(CascadeError::config(
                    "🚫 Currently on main/master branch!\n\n\
                    Cascade hooks are designed for feature branch development.\n\
                    Working directly on main/master with stacked diffs can:\n\
                    • Complicate the commit history\n\
                    • Interfere with team collaboration\n\
                    • Break CI/CD workflows\n\n\
                    💡 Recommended workflow:\n\
                    1. Create a feature branch: git checkout -b feature/my-feature\n\
                    2. Install hooks: cc hooks install\n\
                    3. Develop with stacked commits\n\
                    4. Submit stack: cc stack submit\n\n\
                    Use --force to install anyway (not recommended).".to_string()
                ));
            }
            BranchType::Feature => {
                println!("✅ Feature branch detected - suitable for stacked development");
            }
            BranchType::Unknown => {
                println!("⚠️ Unknown branch type - proceeding with caution");
            }
        }

        Ok(())
    }

    /// Confirm installation with user
    pub fn confirm_installation(&self) -> Result<()> {
        println!("\n📋 Hook Installation Summary:");
        println!("┌─────────────────────┬─────────────────────────────────┐");
        println!("│ Hook                │ Description                     │");
        println!("├─────────────────────┼─────────────────────────────────┤");
        
        let hooks = vec![
            HookType::PostCommit,
            HookType::PrePush,
            HookType::CommitMsg,
            HookType::PrepareCommitMsg,
        ];

        for hook in &hooks {
            println!("│ {:19} │ {:31} │", hook.filename(), hook.description());
        }
        println!("└─────────────────────┴─────────────────────────────────┘");

        println!("\n🔄 These hooks will automatically:");
        println!("• Add commits to your active stack");
        println!("• Validate commit messages");
        println!("• Prevent force pushes that break stack integrity");
        println!("• Add stack context to commit messages");

        use std::io::{self, Write};
        print!("\n❓ Install Cascade hooks? [Y/n]: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        if input.is_empty() || input == "y" || input == "yes" {
            println!("✅ Proceeding with installation");
            Ok(())
        } else {
            Err(CascadeError::config("Installation cancelled by user".to_string()))
        }
    }
}

/// Run hooks management commands
pub async fn install() -> Result<()> {
    install_with_options(false, false, false, false).await
}

pub async fn install_with_options(
    skip_checks: bool, 
    allow_main_branch: bool, 
    yes: bool, 
    force: bool
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;
    
    let hooks_manager = HooksManager::new(&current_dir)?;
    
    let options = InstallOptions {
        check_prerequisites: !skip_checks,
        feature_branches_only: !allow_main_branch,
        confirm: !yes,
        force,
    };
    
    hooks_manager.install_with_options(&options)
}

pub async fn uninstall() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;
    
    let hooks_manager = HooksManager::new(&current_dir)?;
    hooks_manager.uninstall_all()
}

pub async fn status() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;
    
    let hooks_manager = HooksManager::new(&current_dir)?;
    hooks_manager.list_installed_hooks()
}

pub async fn install_hook(hook_name: &str) -> Result<()> {
    install_hook_with_options(hook_name, false, false).await
}

pub async fn install_hook_with_options(hook_name: &str, skip_checks: bool, force: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;
    
    let hooks_manager = HooksManager::new(&current_dir)?;
    
    let hook_type = match hook_name {
        "post-commit" => HookType::PostCommit,
        "pre-push" => HookType::PrePush,
        "commit-msg" => HookType::CommitMsg,
        "prepare-commit-msg" => HookType::PrepareCommitMsg,
        _ => return Err(CascadeError::config(format!("Unknown hook type: {}", hook_name))),
    };
    
    // Run basic validation if not skipped
    if !skip_checks && !force {
        hooks_manager.validate_prerequisites()?;
    }
    
    hooks_manager.install_hook(&hook_type)
}

pub async fn uninstall_hook(hook_name: &str) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;
    
    let hooks_manager = HooksManager::new(&current_dir)?;
    
    let hook_type = match hook_name {
        "post-commit" => HookType::PostCommit,
        "pre-push" => HookType::PrePush,
        "commit-msg" => HookType::CommitMsg,
        "prepare-commit-msg" => HookType::PrepareCommitMsg,
        _ => return Err(CascadeError::config(format!("Unknown hook type: {}", hook_name))),
    };
    
    hooks_manager.uninstall_hook(&hook_type)
} 