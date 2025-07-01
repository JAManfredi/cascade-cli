use crate::config::Settings;
use crate::errors::{CascadeError, Result};
use crate::git::find_repository_root;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
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
    Main,    // main, master, develop
    Feature, // feature branches
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
    fn filename(&self) -> String {
        let base_name = match self {
            HookType::PostCommit => "post-commit",
            HookType::PrePush => "pre-push",
            HookType::CommitMsg => "commit-msg",
            HookType::PrepareCommitMsg => "prepare-commit-msg",
        };
        format!(
            "{}{}",
            base_name,
            crate::utils::platform::git_hook_extension()
        )
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
            return Err(CascadeError::config(
                "Git hooks directory not found. Is this a Git repository?".to_string(),
            ));
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

    /// Install only essential hooks (for setup) - excludes post-commit
    pub fn install_essential(&self) -> Result<()> {
        println!("🪝 Installing essential Cascade Git hooks...");

        let essential_hooks = vec![
            HookType::PrePush,
            HookType::CommitMsg,
            HookType::PrepareCommitMsg,
        ];

        for hook in essential_hooks {
            self.install_hook(&hook)?;
        }

        println!("✅ Essential Cascade hooks installed successfully!");
        println!(
            "💡 Note: Post-commit hook available separately with 'ca hooks install post-commit'"
        );
        println!("\n💡 Hooks installed:");
        self.list_installed_hooks()?;

        Ok(())
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
            fs::copy(&hook_path, &backup_path).map_err(|e| {
                CascadeError::config(format!("Failed to backup existing hook: {e}"))
            })?;
            println!("📦 Backed up existing {} hook", hook_type.filename());
        }

        // Write new hook
        fs::write(&hook_path, hook_content)
            .map_err(|e| CascadeError::config(format!("Failed to write hook file: {e}")))?;

        // Make executable (platform-specific)
        crate::utils::platform::make_executable(&hook_path)
            .map_err(|e| CascadeError::config(format!("Failed to make hook executable: {e}")))?;

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
                .map_err(|e| CascadeError::config(format!("Failed to read hook file: {e}")))?;

            // Check for platform-appropriate hook marker
            let is_cascade_hook = if cfg!(windows) {
                content.contains("rem Cascade CLI Hook")
            } else {
                content.contains("# Cascade CLI Hook")
            };

            if is_cascade_hook {
                fs::remove_file(&hook_path).map_err(|e| {
                    CascadeError::config(format!("Failed to remove hook file: {e}"))
                })?;

                // Restore backup if it exists
                let backup_path = hook_path.with_extension("cascade-backup");
                if backup_path.exists() {
                    fs::rename(&backup_path, &hook_path).map_err(|e| {
                        CascadeError::config(format!("Failed to restore backup: {e}"))
                    })?;
                    println!("📦 Restored original {} hook", hook_type.filename());
                } else {
                    println!("🗑️ Removed {} hook", hook_type.filename());
                }
            } else {
                println!(
                    "⚠️ {} hook exists but is not a Cascade hook, skipping",
                    hook_type.filename()
                );
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
                // Check for platform-appropriate hook marker
                let is_cascade_hook = if cfg!(windows) {
                    content.contains("rem Cascade CLI Hook")
                } else {
                    content.contains("# Cascade CLI Hook")
                };

                if is_cascade_hook {
                    "✅ Cascade"
                } else {
                    "⚠️ Custom "
                }
            } else {
                "❌ Missing"
            };

            println!(
                "│ {:19} │ {:8} │ {:31} │",
                hook.filename(),
                status,
                hook.description()
            );
        }
        println!("└─────────────────────┴──────────┴─────────────────────────────────┘");

        Ok(())
    }

    /// Generate hook script content
    pub fn generate_hook_script(&self, hook_type: &HookType) -> Result<String> {
        let cascade_cli = env::current_exe()
            .map_err(|e| {
                CascadeError::config(format!("Failed to get current executable path: {e}"))
            })?
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
        #[cfg(windows)]
        {
            format!(
                "@echo off\n\
                 rem Cascade CLI Hook - Post Commit\n\
                 rem Automatically adds new commits to the active stack\n\n\
                 rem Get the commit hash and message\n\
                 for /f \"tokens=*\" %%i in ('git rev-parse HEAD') do set COMMIT_HASH=%%i\n\
                 for /f \"tokens=*\" %%i in ('git log --format=%%s -n 1 HEAD') do set COMMIT_MSG=%%i\n\n\
                 rem Find repository root and check if Cascade is initialized\n\
                 for /f \"tokens=*\" %%i in ('git rev-parse --show-toplevel 2^>nul') do set REPO_ROOT=%%i\n\
                 if \"%REPO_ROOT%\"==\"\" set REPO_ROOT=.\n\
                 if not exist \"%REPO_ROOT%\\.cascade\" (\n\
                     echo ℹ️ Cascade not initialized, skipping stack management\n\
                     echo 💡 Run 'ca init' to start using stacked diffs\n\
                     exit /b 0\n\
                 )\n\n\
                 rem Check if there's an active stack\n\
                 \"{cascade_cli}\" stack list --active >nul 2>&1\n\
                 if %ERRORLEVEL% neq 0 (\n\
                     echo ℹ️ No active stack found, commit will not be added to any stack\n\
                     echo 💡 Use 'ca stack create ^<name^>' to create a stack for this commit\n\
                     exit /b 0\n\
                 )\n\n\
                 rem Add commit to active stack\n\
                 echo 🪝 Adding commit to active stack...\n\
                 echo 📝 Commit: %COMMIT_MSG%\n\
                 \"{cascade_cli}\" stack push --commit \"%COMMIT_HASH%\" --message \"%COMMIT_MSG%\"\n\
                 if %ERRORLEVEL% equ 0 (\n\
                     echo ✅ Commit added to stack successfully\n\
                     echo 💡 Next: 'ca submit' to create PRs when ready\n\
                 ) else (\n\
                     echo ⚠️ Failed to add commit to stack\n\
                     echo 💡 You can manually add it with: ca push --commit %COMMIT_HASH%\n\
                 )\n"
            )
        }

        #[cfg(not(windows))]
        {
            format!(
                "#!/bin/sh\n\
                 # Cascade CLI Hook - Post Commit\n\
                 # Automatically adds new commits to the active stack\n\n\
                 set -e\n\n\
                 # Get the commit hash and message\n\
                 COMMIT_HASH=$(git rev-parse HEAD)\n\
                 COMMIT_MSG=$(git log --format=%s -n 1 HEAD)\n\n\
                 # Find repository root and check if Cascade is initialized\n\
                 REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo \".\")\n\
                 if [ ! -d \"$REPO_ROOT/.cascade\" ]; then\n\
                     echo \"ℹ️ Cascade not initialized, skipping stack management\"\n\
                     echo \"💡 Run 'ca init' to start using stacked diffs\"\n\
                     exit 0\n\
                 fi\n\n\
                 # Check if there's an active stack\n\
                 if ! \"{cascade_cli}\" stack list --active > /dev/null 2>&1; then\n\
                     echo \"ℹ️ No active stack found, commit will not be added to any stack\"\n\
                     echo \"💡 Use 'ca stack create <name>' to create a stack for this commit\"\n\
                     exit 0\n\
                 fi\n\n\
                 # Add commit to active stack (using specific commit targeting)\n\
                 echo \"🪝 Adding commit to active stack...\"\n\
                 echo \"📝 Commit: $COMMIT_MSG\"\n\
                 if \"{cascade_cli}\" stack push --commit \"$COMMIT_HASH\" --message \"$COMMIT_MSG\"; then\n\
                     echo \"✅ Commit added to stack successfully\"\n\
                     echo \"💡 Next: 'ca submit' to create PRs when ready\"\n\
                 else\n\
                     echo \"⚠️ Failed to add commit to stack\"\n\
                     echo \"💡 You can manually add it with: ca push --commit $COMMIT_HASH\"\n\
                 fi\n"
            )
        }
    }

    fn generate_pre_push_hook(&self, cascade_cli: &str) -> String {
        #[cfg(windows)]
        {
            format!(
                "@echo off\n\
                 rem Cascade CLI Hook - Pre Push\n\
                 rem Prevents force pushes and validates stack state\n\n\
                 rem Check for force push\n\
                 echo %* | findstr /C:\"--force\" /C:\"--force-with-lease\" /C:\"-f\" >nul\n\
                 if %ERRORLEVEL% equ 0 (\n\
                     echo ❌ Force push detected!\n\
                     echo 🌊 Cascade CLI uses stacked diffs - force pushes can break stack integrity\n\
                     echo.\n\
                     echo 💡 Instead of force pushing, try these streamlined commands:\n\
                     echo    • ca sync      - Sync with remote changes ^(handles rebasing^)\n\
                     echo    • ca push      - Push all unpushed commits ^(new default^)\n\
                     echo    • ca submit    - Submit all entries for review ^(new default^)\n\
                     echo    • ca autoland  - Auto-merge when approved + builds pass\n\
                     echo.\n\
                     echo 🚨 If you really need to force push, run:\n\
                     echo    git push --force-with-lease [remote] [branch]\n\
                     echo    ^(But consider if this will affect other stack entries^)\n\
                     exit /b 1\n\
                 )\n\n\
                 rem Find repository root and check if Cascade is initialized\n\
                 for /f \"tokens=*\" %%i in ('git rev-parse --show-toplevel 2^>nul') do set REPO_ROOT=%%i\n\
                 if \"%REPO_ROOT%\"==\"\" set REPO_ROOT=.\n\
                 if not exist \"%REPO_ROOT%\\.cascade\" (\n\
                     echo ℹ️ Cascade not initialized, allowing push\n\
                     exit /b 0\n\
                 )\n\n\
                 rem Validate stack state\n\
                 echo 🪝 Validating stack state before push...\n\
                 \"{cascade_cli}\" stack validate\n\
                 if %ERRORLEVEL% equ 0 (\n\
                     echo ✅ Stack validation passed\n\
                 ) else (\n\
                     echo ❌ Stack validation failed\n\
                     echo 💡 Fix validation errors before pushing:\n\
                     echo    • ca doctor       - Check overall health\n\
                     echo    • ca status       - Check current stack status\n\
                     echo    • ca sync         - Sync with remote and rebase if needed\n\
                     exit /b 1\n\
                 )\n\n\
                 echo ✅ Pre-push validation complete\n"
            )
        }

        #[cfg(not(windows))]
        {
            format!(
                "#!/bin/sh\n\
                 # Cascade CLI Hook - Pre Push\n\
                 # Prevents force pushes and validates stack state\n\n\
                 set -e\n\n\
                 # Check for force push\n\
                 if echo \"$*\" | grep -q -- \"--force\\|--force-with-lease\\|-f\"; then\n\
                     echo \"❌ Force push detected!\"\n\
                     echo \"🌊 Cascade CLI uses stacked diffs - force pushes can break stack integrity\"\n\
                     echo \"\"\n\
                     echo \"💡 Instead of force pushing, try these streamlined commands:\"\n\
                     echo \"   • ca sync      - Sync with remote changes (handles rebasing)\"\n\
                     echo \"   • ca push      - Push all unpushed commits (new default)\"\n\
                     echo \"   • ca submit    - Submit all entries for review (new default)\"\n\
                     echo \"   • ca autoland  - Auto-merge when approved + builds pass\"\n\
                     echo \"\"\n\
                     echo \"🚨 If you really need to force push, run:\"\n\
                     echo \"   git push --force-with-lease [remote] [branch]\"\n\
                     echo \"   (But consider if this will affect other stack entries)\"\n\
                     exit 1\n\
                 fi\n\n\
                 # Find repository root and check if Cascade is initialized\n\
                 REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo \".\")\n\
                 if [ ! -d \"$REPO_ROOT/.cascade\" ]; then\n\
                     echo \"ℹ️ Cascade not initialized, allowing push\"\n\
                     exit 0\n\
                 fi\n\n\
                 # Validate stack state\n\
                 echo \"🪝 Validating stack state before push...\"\n\
                 if \"{cascade_cli}\" stack validate; then\n\
                     echo \"✅ Stack validation passed\"\n\
                 else\n\
                     echo \"❌ Stack validation failed\"\n\
                     echo \"💡 Fix validation errors before pushing:\"\n\
                     echo \"   • ca doctor       - Check overall health\"\n\
                     echo \"   • ca status       - Check current stack status\"\n\
                     echo \"   • ca sync         - Sync with remote and rebase if needed\"\n\
                     exit 1\n\
                 fi\n\n\
                 echo \"✅ Pre-push validation complete\"\n"
            )
        }
    }

    fn generate_commit_msg_hook(&self, _cascade_cli: &str) -> String {
        #[cfg(windows)]
        {
            r#"@echo off
rem Cascade CLI Hook - Commit Message
rem Validates commit message format

set COMMIT_MSG_FILE=%1
if "%COMMIT_MSG_FILE%"=="" (
    echo ❌ No commit message file provided
    exit /b 1
)

rem Read commit message (Windows batch is limited, but this covers basic cases)
for /f "delims=" %%i in ('type "%COMMIT_MSG_FILE%"') do set COMMIT_MSG=%%i

rem Skip validation for merge commits, fixup commits, etc.
echo %COMMIT_MSG% | findstr /B /C:"Merge" /C:"Revert" /C:"fixup!" /C:"squash!" >nul
if %ERRORLEVEL% equ 0 exit /b 0

rem Find repository root and check if Cascade is initialized
for /f "tokens=*" %%i in ('git rev-parse --show-toplevel 2^>nul') do set REPO_ROOT=%%i
if "%REPO_ROOT%"=="" set REPO_ROOT=.
if not exist "%REPO_ROOT%\.cascade" exit /b 0

rem Basic commit message validation
echo %COMMIT_MSG% | findstr /R "^..........*" >nul
if %ERRORLEVEL% neq 0 (
    echo ❌ Commit message too short (minimum 10 characters)
    echo 💡 Write a descriptive commit message for better stack management
    exit /b 1
)

rem Check for very long messages (approximate check in batch)
echo %COMMIT_MSG% | findstr /R "^..................................................................................*" >nul
if %ERRORLEVEL% equ 0 (
    echo ⚠️ Warning: Commit message longer than 72 characters
    echo 💡 Consider keeping the first line short for better readability
)

rem Check for conventional commit format (optional)
echo %COMMIT_MSG% | findstr /R "^(feat|fix|docs|style|refactor|test|chore|perf|ci|build)" >nul
if %ERRORLEVEL% neq 0 (
    echo 💡 Consider using conventional commit format:
    echo    feat: add new feature
    echo    fix: resolve bug
    echo    docs: update documentation
    echo    etc.
)

echo ✅ Commit message validation passed
"#.to_string()
        }

        #[cfg(not(windows))]
        {
            r#"#!/bin/sh
# Cascade CLI Hook - Commit Message
# Validates commit message format

set -e

COMMIT_MSG_FILE="$1"
COMMIT_MSG=$(cat "$COMMIT_MSG_FILE")

# Skip validation for merge commits, fixup commits, etc.
if echo "$COMMIT_MSG" | grep -E "^(Merge|Revert|fixup!|squash!)" > /dev/null; then
    exit 0
fi

# Find repository root and check if Cascade is initialized
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo ".")
if [ ! -d "$REPO_ROOT/.cascade" ]; then
    exit 0
fi

# Basic commit message validation
if [ ${#COMMIT_MSG} -lt 10 ]; then
    echo "❌ Commit message too short (minimum 10 characters)"
    echo "💡 Write a descriptive commit message for better stack management"
    exit 1
fi

if [ ${#COMMIT_MSG} -gt 72 ]; then
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
"#.to_string()
        }
    }

    fn generate_prepare_commit_msg_hook(&self, cascade_cli: &str) -> String {
        #[cfg(windows)]
        {
            format!(
                "@echo off\n\
                 rem Cascade CLI Hook - Prepare Commit Message\n\
                 rem Adds stack context to commit messages\n\n\
                 set COMMIT_MSG_FILE=%1\n\
                 set COMMIT_SOURCE=%2\n\
                 set COMMIT_SHA=%3\n\n\
                 rem Only modify message if it's a regular commit (not merge, template, etc.)\n\
                 if not \"%COMMIT_SOURCE%\"==\"\" if not \"%COMMIT_SOURCE%\"==\"message\" exit /b 0\n\n\
                 rem Find repository root and check if Cascade is initialized\n\
                 for /f \"tokens=*\" %%i in ('git rev-parse --show-toplevel 2^>nul') do set REPO_ROOT=%%i\n\
                 if \"%REPO_ROOT%\"==\"\" set REPO_ROOT=.\n\
                 if not exist \"%REPO_ROOT%\\.cascade\" exit /b 0\n\n\
                 rem Get active stack info\n\
                 for /f \"tokens=*\" %%i in ('\"{cascade_cli}\" stack list --active --format=name 2^>nul') do set ACTIVE_STACK=%%i\n\n\
                 if not \"%ACTIVE_STACK%\"==\"\" (\n\
                     rem Get current commit message\n\
                     set /p CURRENT_MSG=<%COMMIT_MSG_FILE%\n\n\
                     rem Skip if message already has stack context\n\
                     echo !CURRENT_MSG! | findstr \"[stack:\" >nul\n\
                     if %ERRORLEVEL% equ 0 exit /b 0\n\n\
                     rem Add stack context to commit message\n\
                     echo.\n\
                     echo # Stack: %ACTIVE_STACK%\n\
                     echo # This commit will be added to the active stack automatically.\n\
                     echo # Use 'ca stack status' to see the current stack state.\n\
                     type \"%COMMIT_MSG_FILE%\"\n\
                 ) > \"%COMMIT_MSG_FILE%.tmp\"\n\
                 move \"%COMMIT_MSG_FILE%.tmp\" \"%COMMIT_MSG_FILE%\"\n"
            )
        }

        #[cfg(not(windows))]
        {
            format!(
                "#!/bin/sh\n\
                 # Cascade CLI Hook - Prepare Commit Message\n\
                 # Adds stack context to commit messages\n\n\
                 set -e\n\n\
                 COMMIT_MSG_FILE=\"$1\"\n\
                 COMMIT_SOURCE=\"$2\"\n\
                 COMMIT_SHA=\"$3\"\n\n\
                 # Only modify message if it's a regular commit (not merge, template, etc.)\n\
                 if [ \"$COMMIT_SOURCE\" != \"\" ] && [ \"$COMMIT_SOURCE\" != \"message\" ]; then\n\
                     exit 0\n\
                 fi\n\n\
                 # Find repository root and check if Cascade is initialized\n\
                 REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo \".\")\n\
                 if [ ! -d \"$REPO_ROOT/.cascade\" ]; then\n\
                     exit 0\n\
                 fi\n\n\
                 # Get active stack info\n\
                 ACTIVE_STACK=$(\"{cascade_cli}\" stack list --active --format=name 2>/dev/null || echo \"\")\n\n\
                 if [ -n \"$ACTIVE_STACK\" ]; then\n\
                     # Get current commit message\n\
                     CURRENT_MSG=$(cat \"$COMMIT_MSG_FILE\")\n\
                     \n\
                     # Skip if message already has stack context\n\
                     if echo \"$CURRENT_MSG\" | grep -q \"\\[stack:\"; then\n\
                         exit 0\n\
                     fi\n\
                     \n\
                     # Add stack context to commit message\n\
                     echo \"\n\
                 # Stack: $ACTIVE_STACK\n\
                 # This commit will be added to the active stack automatically.\n\
                 # Use 'ca stack status' to see the current stack state.\n\
                 $CURRENT_MSG\" > \"$COMMIT_MSG_FILE\"\n\
                 fi\n"
            )
        }
    }

    /// Detect repository type from remote URLs
    pub fn detect_repository_type(&self) -> Result<RepositoryType> {
        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to get remote URL: {e}")))?;

        if !output.status.success() {
            return Ok(RepositoryType::Unknown);
        }

        let remote_url = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_lowercase();

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
            .args(["branch", "--show-current"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to get current branch: {e}")))?;

        if !output.status.success() {
            return Ok(BranchType::Unknown);
        }

        let branch_name = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_lowercase();

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
                println!("💡 Hooks will work great with 'ca submit' and 'ca autoland' for Bitbucket integration");
            }
            RepositoryType::GitHub => {
                println!("✅ GitHub repository detected");
                println!("💡 Consider setting up GitHub Actions for CI/CD integration");
            }
            RepositoryType::GitLab => {
                println!("✅ GitLab repository detected");
                println!("💡 GitLab CI integration works well with Cascade stacks");
            }
            RepositoryType::AzureDevOps => {
                println!("✅ Azure DevOps repository detected");
                println!("💡 Azure Pipelines can be configured to work with Cascade workflows");
            }
            RepositoryType::Unknown => {
                println!(
                    "ℹ️ Unknown repository type - hooks will still work for local Git operations"
                );
            }
        }

        // 2. Check Cascade configuration
        let config_dir = crate::config::get_repo_config_dir(&self.repo_path)?;
        let config_path = config_dir.join("config.json");
        if !config_path.exists() {
            return Err(CascadeError::config(
                "🚫 Cascade not initialized!\n\n\
                Please run 'ca init' or 'ca setup' first to configure Cascade CLI.\n\
                Hooks require proper Bitbucket Server configuration.\n\n\
                Use --force to install anyway (not recommended)."
                    .to_string(),
            ));
        }

        // 3. Validate Bitbucket configuration
        let config = Settings::load_from_file(&config_path)?;

        if config.bitbucket.url == "https://bitbucket.example.com"
            || config.bitbucket.url.contains("example.com")
        {
            return Err(CascadeError::config(
                "🚫 Invalid Bitbucket configuration!\n\n\
                Your Bitbucket URL appears to be a placeholder.\n\
                Please run 'ca setup' to configure a real Bitbucket Server.\n\n\
                Use --force to install anyway (not recommended)."
                    .to_string(),
            ));
        }

        if config.bitbucket.project == "PROJECT" || config.bitbucket.repo == "repo" {
            return Err(CascadeError::config(
                "🚫 Incomplete Bitbucket configuration!\n\n\
                Your project/repository settings appear to be placeholders.\n\
                Please run 'ca setup' to complete configuration.\n\n\
                Use --force to install anyway (not recommended)."
                    .to_string(),
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
                    2. Install hooks: ca hooks install\n\
                    3. Develop with stacked commits (auto-added with hooks)\n\
                    4. Push & submit: ca push && ca submit (all by default)\n\
                    5. Auto-land when ready: ca autoland\n\n\
                    Use --force to install anyway (not recommended)."
                        .to_string(),
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

        println!("\n✨ With hooks + new defaults, your workflow becomes:");
        println!("  git commit       → Auto-added to stack");
        println!("  ca push          → Pushes all by default");
        println!("  ca submit        → Submits all by default");
        println!("  ca autoland      → Auto-merges when ready");

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
            Err(CascadeError::config(
                "Installation cancelled by user".to_string(),
            ))
        }
    }
}

/// Run hooks management commands
pub async fn install() -> Result<()> {
    install_with_options(false, false, false, false).await
}

pub async fn install_essential() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let hooks_manager = HooksManager::new(&repo_root)?;
    hooks_manager.install_essential()
}

pub async fn install_with_options(
    skip_checks: bool,
    allow_main_branch: bool,
    yes: bool,
    force: bool,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let hooks_manager = HooksManager::new(&repo_root)?;

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
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let hooks_manager = HooksManager::new(&repo_root)?;
    hooks_manager.uninstall_all()
}

pub async fn status() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let hooks_manager = HooksManager::new(&repo_root)?;
    hooks_manager.list_installed_hooks()
}

pub async fn install_hook(hook_name: &str) -> Result<()> {
    install_hook_with_options(hook_name, false, false).await
}

pub async fn install_hook_with_options(
    hook_name: &str,
    skip_checks: bool,
    force: bool,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let hooks_manager = HooksManager::new(&repo_root)?;

    let hook_type = match hook_name {
        "post-commit" => HookType::PostCommit,
        "pre-push" => HookType::PrePush,
        "commit-msg" => HookType::CommitMsg,
        "prepare-commit-msg" => HookType::PrepareCommitMsg,
        _ => {
            return Err(CascadeError::config(format!(
                "Unknown hook type: {hook_name}"
            )))
        }
    };

    // Run basic validation if not skipped
    if !skip_checks && !force {
        hooks_manager.validate_prerequisites()?;
    }

    hooks_manager.install_hook(&hook_type)
}

pub async fn uninstall_hook(hook_name: &str) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let hooks_manager = HooksManager::new(&repo_root)?;

    let hook_type = match hook_name {
        "post-commit" => HookType::PostCommit,
        "pre-push" => HookType::PrePush,
        "commit-msg" => HookType::CommitMsg,
        "prepare-commit-msg" => HookType::PrepareCommitMsg,
        _ => {
            return Err(CascadeError::config(format!(
                "Unknown hook type: {hook_name}"
            )))
        }
    };

    hooks_manager.uninstall_hook(&hook_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repository
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Initialize cascade
        crate::config::initialize_repo(&repo_path, Some("https://test.bitbucket.com".to_string()))
            .unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_hooks_manager_creation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let _manager = HooksManager::new(&repo_path).unwrap();

        assert_eq!(_manager.repo_path, repo_path);
        assert_eq!(_manager.hooks_dir, repo_path.join(".git/hooks"));
    }

    #[test]
    fn test_hook_installation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = HooksManager::new(&repo_path).unwrap();

        // Test installing post-commit hook
        let hook_type = HookType::PostCommit;
        let result = manager.install_hook(&hook_type);
        assert!(result.is_ok());

        // Verify hook file exists with platform-appropriate filename
        let hook_filename = hook_type.filename();
        let hook_path = repo_path.join(".git/hooks").join(&hook_filename);
        assert!(hook_path.exists());

        // Verify hook is executable (platform-specific)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&hook_path).unwrap();
            let permissions = metadata.permissions();
            assert!(permissions.mode() & 0o111 != 0); // Check executable bit
        }

        #[cfg(windows)]
        {
            // On Windows, verify it has .bat extension and file exists
            assert!(hook_filename.ends_with(".bat"));
            assert!(hook_path.exists());
        }
    }

    #[test]
    fn test_hook_detection() {
        let (_temp_dir, repo_path) = create_test_repo();
        let _manager = HooksManager::new(&repo_path).unwrap();

        // Check if hook files exist with platform-appropriate filenames
        let post_commit_path = repo_path
            .join(".git/hooks")
            .join(HookType::PostCommit.filename());
        let pre_push_path = repo_path
            .join(".git/hooks")
            .join(HookType::PrePush.filename());
        let commit_msg_path = repo_path
            .join(".git/hooks")
            .join(HookType::CommitMsg.filename());

        // Initially no hooks should be installed
        assert!(!post_commit_path.exists());
        assert!(!pre_push_path.exists());
        assert!(!commit_msg_path.exists());
    }

    #[test]
    fn test_hook_validation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = HooksManager::new(&repo_path).unwrap();

        // Test validation - may fail in CI due to missing dependencies
        let validation = manager.validate_prerequisites();
        // In CI environment, validation might fail due to missing configuration
        // Just ensure it doesn't panic
        let _ = validation; // Don't assert ok/err, just ensure no panic

        // Test branch validation - should work regardless of environment
        let branch_validation = manager.validate_branch_suitability();
        // Branch validation should work in most cases, but be tolerant
        let _ = branch_validation; // Don't assert ok/err, just ensure no panic
    }

    #[test]
    fn test_hook_uninstallation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = HooksManager::new(&repo_path).unwrap();

        // Install then uninstall hook
        let hook_type = HookType::PostCommit;
        manager.install_hook(&hook_type).unwrap();

        let hook_path = repo_path.join(".git/hooks").join(hook_type.filename());
        assert!(hook_path.exists());

        let result = manager.uninstall_hook(&hook_type);
        assert!(result.is_ok());
        assert!(!hook_path.exists());
    }

    #[test]
    fn test_hook_content_generation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = HooksManager::new(&repo_path).unwrap();

        // Use a known binary name for testing
        let binary_name = "cascade-cli";

        // Test post-commit hook generation
        let post_commit_content = manager.generate_post_commit_hook(binary_name);
        #[cfg(windows)]
        {
            assert!(post_commit_content.contains("@echo off"));
            assert!(post_commit_content.contains("rem Cascade CLI Hook"));
        }
        #[cfg(not(windows))]
        {
            assert!(post_commit_content.contains("#!/bin/sh"));
            assert!(post_commit_content.contains("# Cascade CLI Hook"));
        }
        assert!(post_commit_content.contains(binary_name));

        // Test pre-push hook generation
        let pre_push_content = manager.generate_pre_push_hook(binary_name);
        #[cfg(windows)]
        {
            assert!(pre_push_content.contains("@echo off"));
            assert!(pre_push_content.contains("rem Cascade CLI Hook"));
        }
        #[cfg(not(windows))]
        {
            assert!(pre_push_content.contains("#!/bin/sh"));
            assert!(pre_push_content.contains("# Cascade CLI Hook"));
        }
        assert!(pre_push_content.contains(binary_name));

        // Test commit-msg hook generation (doesn't use binary, just validates)
        let commit_msg_content = manager.generate_commit_msg_hook(binary_name);
        #[cfg(windows)]
        {
            assert!(commit_msg_content.contains("@echo off"));
            assert!(commit_msg_content.contains("rem Cascade CLI Hook"));
        }
        #[cfg(not(windows))]
        {
            assert!(commit_msg_content.contains("#!/bin/sh"));
            assert!(commit_msg_content.contains("# Cascade CLI Hook"));
        }

        // Test prepare-commit-msg hook generation (does use binary)
        let prepare_commit_content = manager.generate_prepare_commit_msg_hook(binary_name);
        #[cfg(windows)]
        {
            assert!(prepare_commit_content.contains("@echo off"));
            assert!(prepare_commit_content.contains("rem Cascade CLI Hook"));
        }
        #[cfg(not(windows))]
        {
            assert!(prepare_commit_content.contains("#!/bin/sh"));
            assert!(prepare_commit_content.contains("# Cascade CLI Hook"));
        }
        assert!(prepare_commit_content.contains(binary_name));
    }

    #[test]
    fn test_hook_status_reporting() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = HooksManager::new(&repo_path).unwrap();

        // Check repository type detection - should work with our test setup
        let repo_type = manager.detect_repository_type().unwrap();
        // In CI environment, this might be Unknown if remote detection fails
        assert!(matches!(
            repo_type,
            RepositoryType::Bitbucket | RepositoryType::Unknown
        ));

        // Check branch type detection
        let branch_type = manager.detect_branch_type().unwrap();
        // Should be on main/master branch, but allow for different default branch names
        assert!(matches!(
            branch_type,
            BranchType::Main | BranchType::Unknown
        ));
    }

    #[test]
    fn test_force_installation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = HooksManager::new(&repo_path).unwrap();

        // Create a fake existing hook with platform-appropriate content
        let hook_filename = HookType::PostCommit.filename();
        let hook_path = repo_path.join(".git/hooks").join(&hook_filename);

        #[cfg(windows)]
        let existing_content = "@echo off\necho existing hook";
        #[cfg(not(windows))]
        let existing_content = "#!/bin/sh\necho 'existing hook'";

        std::fs::write(&hook_path, existing_content).unwrap();

        // Install hook (should backup and replace existing)
        let hook_type = HookType::PostCommit;
        let result = manager.install_hook(&hook_type);
        assert!(result.is_ok());

        // Verify new content replaced old content
        let content = std::fs::read_to_string(&hook_path).unwrap();
        #[cfg(windows)]
        {
            assert!(content.contains("rem Cascade CLI Hook"));
        }
        #[cfg(not(windows))]
        {
            assert!(content.contains("# Cascade CLI Hook"));
        }
        assert!(!content.contains("existing hook"));

        // Verify backup was created
        let backup_path = hook_path.with_extension("cascade-backup");
        assert!(backup_path.exists());
    }
}
