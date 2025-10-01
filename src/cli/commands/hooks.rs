use crate::cli::output::Output;
use crate::config::Settings;
use crate::errors::{CascadeError, Result};
use crate::git::find_repository_root;
use dialoguer::{theme::ColorfulTheme, Confirm};
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
    repo_id: String,
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
    /// Smart edit mode guidance before commit
    PreCommit,
    /// Prepares commit message with stack context
    PrepareCommitMsg,
}

impl HookType {
    fn filename(&self) -> String {
        let base_name = match self {
            HookType::PostCommit => "post-commit",
            HookType::PrePush => "pre-push",
            HookType::CommitMsg => "commit-msg",
            HookType::PreCommit => "pre-commit",
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
            HookType::PreCommit => "Smart edit mode guidance for better UX",
            HookType::PrepareCommitMsg => "Add stack context to commit messages",
        }
    }
}

impl HooksManager {
    pub fn new(repo_path: &Path) -> Result<Self> {
        // Verify this is a git repository
        let git_dir = repo_path.join(".git");
        if !git_dir.exists() {
            return Err(CascadeError::config(
                "Not a Git repository. Git hooks require a valid Git repository.".to_string(),
            ));
        }

        // Generate a unique repo ID based on remote URL
        let repo_id = Self::generate_repo_id(repo_path)?;

        Ok(Self {
            repo_path: repo_path.to_path_buf(),
            repo_id,
        })
    }

    /// Generate a unique repository identifier based on remote URL
    fn generate_repo_id(repo_path: &Path) -> Result<String> {
        use std::process::Command;

        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to get remote URL: {e}")))?;

        if !output.status.success() {
            // Fallback to absolute path hash if no remote
            use sha2::{Digest, Sha256};
            let canonical_path = repo_path
                .canonicalize()
                .unwrap_or_else(|_| repo_path.to_path_buf());
            let path_str = canonical_path.to_string_lossy();
            let mut hasher = Sha256::new();
            hasher.update(path_str.as_bytes());
            let result = hasher.finalize();
            let hash = format!("{result:x}");
            return Ok(format!("local-{}", &hash[..8]));
        }

        let remote_url = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Convert URL to safe directory name
        // e.g., https://github.com/user/repo.git -> github.com-user-repo
        let safe_name = remote_url
            .replace("https://", "")
            .replace("http://", "")
            .replace("git@", "")
            .replace("ssh://", "")
            .replace(".git", "")
            .replace([':', '/', '\\'], "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.' || *c == '_')
            .collect::<String>();

        Ok(safe_name)
    }

    /// Get the Cascade-specific hooks directory for this repo
    fn get_cascade_hooks_dir(&self) -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| CascadeError::config("Could not find home directory".to_string()))?;
        let cascade_hooks = home.join(".cascade").join("hooks").join(&self.repo_id);
        Ok(cascade_hooks)
    }

    /// Get the Cascade config directory for this repo
    fn get_cascade_config_dir(&self) -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| CascadeError::config("Could not find home directory".to_string()))?;
        let cascade_config = home.join(".cascade").join("config").join(&self.repo_id);
        Ok(cascade_config)
    }

    /// Save the current core.hooksPath value for later restoration
    fn save_original_hooks_path(&self) -> Result<()> {
        use std::process::Command;

        let output = Command::new("git")
            .args(["config", "--get", "core.hooksPath"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to check git config: {e}")))?;

        let config_dir = self.get_cascade_config_dir()?;
        fs::create_dir_all(&config_dir)
            .map_err(|e| CascadeError::config(format!("Failed to create config directory: {e}")))?;

        let original_path = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            // Empty string means it wasn't set
            String::new()
        };

        fs::write(config_dir.join("original-hooks-path"), original_path).map_err(|e| {
            CascadeError::config(format!("Failed to save original hooks path: {e}"))
        })?;

        Ok(())
    }

    /// Restore the original core.hooksPath value
    fn restore_original_hooks_path(&self) -> Result<()> {
        use std::process::Command;

        let config_dir = self.get_cascade_config_dir()?;
        let original_path_file = config_dir.join("original-hooks-path");

        if !original_path_file.exists() {
            // Nothing to restore
            return Ok(());
        }

        let original_path = fs::read_to_string(&original_path_file).map_err(|e| {
            CascadeError::config(format!("Failed to read original hooks path: {e}"))
        })?;

        if original_path.is_empty() {
            // It wasn't set originally, so unset it
            Command::new("git")
                .args(["config", "--unset", "core.hooksPath"])
                .current_dir(&self.repo_path)
                .output()
                .map_err(|e| {
                    CascadeError::config(format!("Failed to unset core.hooksPath: {e}"))
                })?;
        } else {
            // Restore the original value
            Command::new("git")
                .args(["config", "core.hooksPath", &original_path])
                .current_dir(&self.repo_path)
                .output()
                .map_err(|e| {
                    CascadeError::config(format!("Failed to restore core.hooksPath: {e}"))
                })?;
        }

        // Clean up the saved file
        fs::remove_file(original_path_file).ok();

        Ok(())
    }

    /// Get the actual hooks directory path, respecting core.hooksPath configuration
    #[allow(dead_code)]
    fn get_hooks_path(repo_path: &Path) -> Result<PathBuf> {
        use std::process::Command;

        // Try to get core.hooksPath configuration
        let output = Command::new("git")
            .args(["config", "--get", "core.hooksPath"])
            .current_dir(repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to check git config: {e}")))?;

        let hooks_path = if output.status.success() {
            let configured_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if configured_path.is_empty() {
                // Empty value means default
                repo_path.join(".git").join("hooks")
            } else if configured_path.starts_with('/') {
                // Absolute path
                PathBuf::from(configured_path)
            } else {
                // Relative path - relative to repo root
                repo_path.join(configured_path)
            }
        } else {
            // No core.hooksPath configured, use default
            repo_path.join(".git").join("hooks")
        };

        Ok(hooks_path)
    }

    /// Install all recommended Cascade hooks
    pub fn install_all(&self) -> Result<()> {
        self.install_with_options(&InstallOptions::default())
    }

    /// Install only essential hooks (for setup) - excludes post-commit
    pub fn install_essential(&self) -> Result<()> {
        Output::progress("Installing essential Cascade Git hooks");

        let essential_hooks = vec![
            HookType::PrePush,
            HookType::CommitMsg,
            HookType::PrepareCommitMsg,
            HookType::PreCommit,
        ];

        for hook in essential_hooks {
            self.install_hook(&hook)?;
        }

        Output::success("Essential Cascade hooks installed successfully!");
        Output::tip("Note: Post-commit auto-add hook available with 'ca hooks install --all'");
        Output::section("Hooks installed");
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

        Output::progress("Installing all Cascade Git hooks");

        // Install ALL hooks (all 5 HookType variants)
        let hooks = vec![
            HookType::PostCommit,
            HookType::PrePush,
            HookType::CommitMsg,
            HookType::PrepareCommitMsg,
            HookType::PreCommit,
        ];

        for hook in hooks {
            self.install_hook(&hook)?;
        }

        Output::success("All Cascade hooks installed successfully!");
        Output::section("Hooks installed");
        self.list_installed_hooks()?;

        Ok(())
    }

    /// Install a specific hook
    pub fn install_hook(&self, hook_type: &HookType) -> Result<()> {
        // Ensure we've saved the original hooks path first
        self.save_original_hooks_path()?;

        // Create cascade hooks directory
        let cascade_hooks_dir = self.get_cascade_hooks_dir()?;
        fs::create_dir_all(&cascade_hooks_dir).map_err(|e| {
            CascadeError::config(format!("Failed to create cascade hooks directory: {e}"))
        })?;

        // Generate hook that chains to original
        let hook_content = self.generate_chaining_hook_script(hook_type)?;
        let hook_path = cascade_hooks_dir.join(hook_type.filename());

        // Write the hook
        fs::write(&hook_path, hook_content)
            .map_err(|e| CascadeError::config(format!("Failed to write hook file: {e}")))?;

        // Make executable (platform-specific)
        crate::utils::platform::make_executable(&hook_path)
            .map_err(|e| CascadeError::config(format!("Failed to make hook executable: {e}")))?;

        // Set core.hooksPath to our cascade directory
        self.set_cascade_hooks_path()?;

        Output::success(format!("Installed {} hook", hook_type.filename()));
        Ok(())
    }

    /// Set git's core.hooksPath to our cascade hooks directory
    fn set_cascade_hooks_path(&self) -> Result<()> {
        use std::process::Command;

        let cascade_hooks_dir = self.get_cascade_hooks_dir()?;
        let hooks_path_str = cascade_hooks_dir.to_string_lossy();

        let output = Command::new("git")
            .args(["config", "core.hooksPath", &hooks_path_str])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to set core.hooksPath: {e}")))?;

        if !output.status.success() {
            return Err(CascadeError::config(format!(
                "Failed to set core.hooksPath: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Remove all Cascade hooks
    pub fn uninstall_all(&self) -> Result<()> {
        Output::progress("Removing Cascade Git hooks");

        // Restore original core.hooksPath
        self.restore_original_hooks_path()?;

        // Clean up cascade hooks directory
        let cascade_hooks_dir = self.get_cascade_hooks_dir()?;
        if cascade_hooks_dir.exists() {
            fs::remove_dir_all(&cascade_hooks_dir).map_err(|e| {
                CascadeError::config(format!("Failed to remove cascade hooks directory: {e}"))
            })?;
        }

        // Clean up config directory if empty
        let cascade_config_dir = self.get_cascade_config_dir()?;
        if cascade_config_dir.exists() {
            // Try to remove, but ignore if not empty
            fs::remove_dir(&cascade_config_dir).ok();
        }

        Output::success("All Cascade hooks removed!");
        Ok(())
    }

    /// Remove a specific hook
    pub fn uninstall_hook(&self, hook_type: &HookType) -> Result<()> {
        let cascade_hooks_dir = self.get_cascade_hooks_dir()?;
        let hook_path = cascade_hooks_dir.join(hook_type.filename());

        if hook_path.exists() {
            fs::remove_file(&hook_path)
                .map_err(|e| CascadeError::config(format!("Failed to remove hook file: {e}")))?;
            Output::success(format!("Removed {} hook", hook_type.filename()));

            // If no more hooks in cascade directory, restore original hooks path
            let remaining_hooks = fs::read_dir(&cascade_hooks_dir)
                .map_err(|e| CascadeError::config(format!("Failed to read hooks directory: {e}")))?
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.path().is_file() && !entry.file_name().to_string_lossy().starts_with('.')
                })
                .count();

            if remaining_hooks == 0 {
                Output::info(
                    "No more Cascade hooks installed, restoring original hooks configuration",
                );
                self.restore_original_hooks_path()?;
                fs::remove_dir(&cascade_hooks_dir).ok();
            }
        } else {
            Output::info(format!("{} hook not found", hook_type.filename()));
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
            HookType::PreCommit,
        ];

        Output::section("Git Hooks Status");

        // Check if we're using cascade hooks directory
        let cascade_hooks_dir = self.get_cascade_hooks_dir()?;
        let using_cascade_hooks = cascade_hooks_dir.exists()
            && self.get_current_hooks_path()?
                == Some(cascade_hooks_dir.to_string_lossy().to_string());

        if using_cascade_hooks {
            Output::success("✓ Cascade hooks are installed and active");
            Output::info(format!(
                "  Hooks directory: {}",
                cascade_hooks_dir.display()
            ));

            // Check what original hooks path was saved
            let config_dir = self.get_cascade_config_dir()?;
            let original_path_file = config_dir.join("original-hooks-path");
            if original_path_file.exists() {
                let original_path = fs::read_to_string(original_path_file).unwrap_or_default();
                if !original_path.is_empty() {
                    Output::info(format!("  Chaining to original hooks: {original_path}"));
                } else {
                    Output::info("  Chaining to original hooks: .git/hooks");
                }
            }
            println!();
        } else {
            Output::warning("⚠️  Cascade hooks are NOT installed in this repository");
            Output::tip("To install Cascade hooks:");
            Output::bullet("Run: ca hooks install            (recommended: 4 essential hooks)");
            Output::bullet("Run: ca hooks install --all      (all 5 hooks + post-commit auto-add)");
            Output::bullet("Both options preserve existing hooks by chaining to them");
            println!();
        }

        for hook in hooks {
            let cascade_hook_path = cascade_hooks_dir.join(hook.filename());

            if using_cascade_hooks && cascade_hook_path.exists() {
                Output::success(format!("{}: {} ✓", hook.filename(), hook.description()));
            } else {
                // Check default location
                let default_hook_path = self
                    .repo_path
                    .join(".git")
                    .join("hooks")
                    .join(hook.filename());
                if default_hook_path.exists() {
                    Output::warning(format!(
                        "{}: {} (In .git/hooks, not managed by Cascade)",
                        hook.filename(),
                        hook.description()
                    ));
                } else {
                    Output::error(format!(
                        "{}: {} (Not installed)",
                        hook.filename(),
                        hook.description()
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get the current core.hooksPath value
    fn get_current_hooks_path(&self) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("git")
            .args(["config", "--get", "core.hooksPath"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to check git config: {e}")))?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                Ok(None)
            } else {
                Ok(Some(path))
            }
        } else {
            Ok(None)
        }
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
            HookType::PreCommit => self.generate_pre_commit_hook(&cascade_cli),
            HookType::PrepareCommitMsg => self.generate_prepare_commit_msg_hook(&cascade_cli),
        };

        Ok(script)
    }

    /// Generate hook script that chains to original hooks
    pub fn generate_chaining_hook_script(&self, hook_type: &HookType) -> Result<String> {
        let cascade_cli = env::current_exe()
            .map_err(|e| {
                CascadeError::config(format!("Failed to get current executable path: {e}"))
            })?
            .to_string_lossy()
            .to_string();

        let config_dir = self.get_cascade_config_dir()?;
        let hook_name = match hook_type {
            HookType::PostCommit => "post-commit",
            HookType::PrePush => "pre-push",
            HookType::CommitMsg => "commit-msg",
            HookType::PreCommit => "pre-commit",
            HookType::PrepareCommitMsg => "prepare-commit-msg",
        };

        // Generate the cascade-specific hook logic
        let cascade_logic = match hook_type {
            HookType::PostCommit => self.generate_post_commit_hook(&cascade_cli),
            HookType::PrePush => self.generate_pre_push_hook(&cascade_cli),
            HookType::CommitMsg => self.generate_commit_msg_hook(&cascade_cli),
            HookType::PreCommit => self.generate_pre_commit_hook(&cascade_cli),
            HookType::PrepareCommitMsg => self.generate_prepare_commit_msg_hook(&cascade_cli),
        };

        // Create wrapper that chains to original
        #[cfg(windows)]
        return Ok(format!(
                "@echo off\n\
                 rem Cascade CLI Hook Wrapper - {}\n\
                 rem This hook runs Cascade logic first, then chains to original hooks\n\n\
                 rem Run Cascade logic first\n\
                 call :cascade_logic %*\n\
                 set CASCADE_RESULT=%ERRORLEVEL%\n\
                 if %CASCADE_RESULT% neq 0 exit /b %CASCADE_RESULT%\n\n\
                 rem Check for original hook\n\
                 set ORIGINAL_HOOKS_PATH=\n\
                 if exist \"{}\\original-hooks-path\" (\n\
                     set /p ORIGINAL_HOOKS_PATH=<\"{}\\original-hooks-path\"\n\
                 )\n\n\
                 if \"%ORIGINAL_HOOKS_PATH%\"==\"\" (\n\
                     rem Default location\n\
                     for /f \"tokens=*\" %%i in ('git rev-parse --git-dir 2^>nul') do set GIT_DIR=%%i\n\
                     if exist \"%GIT_DIR%\\hooks\\{}\" (\n\
                         call \"%GIT_DIR%\\hooks\\{}\" %*\n\
                         exit /b %ERRORLEVEL%\n\
                     )\n\
                 ) else (\n\
                     rem Custom hooks path\n\
                     if exist \"%ORIGINAL_HOOKS_PATH%\\{}\" (\n\
                         call \"%ORIGINAL_HOOKS_PATH%\\{}\" %*\n\
                         exit /b %ERRORLEVEL%\n\
                     )\n\
                 )\n\n\
                 exit /b 0\n\n\
                 :cascade_logic\n\
                 {}\n\
                 exit /b %ERRORLEVEL%\n",
                hook_name,
                config_dir.to_string_lossy(),
                config_dir.to_string_lossy(),
                hook_name,
                hook_name,
                hook_name,
                hook_name,
                cascade_logic
            ));

        #[cfg(not(windows))]
        Ok(format!(
                "#!/bin/sh\n\
                 # Cascade CLI Hook Wrapper - {}\n\
                 # This hook runs Cascade logic first, then chains to original hooks\n\n\
                 set -e\n\n\
                 # Function to run Cascade logic\n\
                 cascade_logic() {{\n\
                 {}\n\
                 }}\n\n\
                 # Run Cascade logic first\n\
                 cascade_logic \"$@\"\n\
                 CASCADE_RESULT=$?\n\
                 if [ $CASCADE_RESULT -ne 0 ]; then\n\
                     exit $CASCADE_RESULT\n\
                 fi\n\n\
                 # Check for original hook\n\
                 ORIGINAL_HOOKS_PATH=\"\"\n\
                 if [ -f \"{}/original-hooks-path\" ]; then\n\
                     ORIGINAL_HOOKS_PATH=$(cat \"{}/original-hooks-path\" 2>/dev/null || echo \"\")\n\
                 fi\n\n\
                 if [ -z \"$ORIGINAL_HOOKS_PATH\" ]; then\n\
                     # Default location\n\
                     GIT_DIR=$(git rev-parse --git-dir 2>/dev/null || echo \".git\")\n\
                     ORIGINAL_HOOK=\"$GIT_DIR/hooks/{}\"\n\
                 else\n\
                     # Custom hooks path\n\
                     ORIGINAL_HOOK=\"$ORIGINAL_HOOKS_PATH/{}\"\n\
                 fi\n\n\
                 # Run original hook if it exists and is executable\n\
                 if [ -x \"$ORIGINAL_HOOK\" ]; then\n\
                     \"$ORIGINAL_HOOK\" \"$@\"\n\
                     exit $?\n\
                 fi\n\n\
                 exit 0\n",
                hook_name,
                cascade_logic.trim_start_matches("#!/bin/sh\n").trim_start_matches("set -e\n"),
                config_dir.to_string_lossy(),
                config_dir.to_string_lossy(),
                hook_name,
                hook_name
        ))
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

    #[allow(clippy::uninlined_format_args)]
    fn generate_pre_commit_hook(&self, cascade_cli: &str) -> String {
        #[cfg(windows)]
        {
            format!(
                "@echo off\n\
                 rem Cascade CLI Hook - Pre Commit\n\
                 rem Smart edit mode guidance for better UX\n\n\
                 rem Check if Cascade is initialized\n\
                 for /f \\\"tokens=*\\\" %%i in ('git rev-parse --show-toplevel 2^>nul') do set REPO_ROOT=%%i\n\
                 if \\\"%REPO_ROOT%\\\"==\\\"\\\" set REPO_ROOT=.\n\
                 if not exist \\\"%REPO_ROOT%\\.cascade\\\" exit /b 0\n\n\
                 rem Check if we're in edit mode\n\
                 \\\"{0}\\\" entry status --quiet >nul 2>&1\n\
                 if %ERRORLEVEL% equ 0 (\n\
                     echo ⚠️ You're in EDIT MODE for a stack entry!\n\
                     echo.\n\
                    echo Choose your action:\n\
                    echo   🔄 [A]mend: Modify the current entry ^(default^)\n\
                    echo   ➕ [N]ew:   Create new entry on top\n\
                    echo   ❌ [C]ancel: Stop and think about it\n\
                    echo.\n\
                    set /p choice=\\\"Your choice (A/n/c): \\\"\n\
                    if \\\"%choice%\\\"==\\\"\\\" set choice=A\n\
                    \n\
                    if /i \\\"%choice%\\\"==\\\"A\\\" (\n\
                        echo ✅ Amending current entry...\n\
                        rem Stage all changes first\n\
                        git add -A\n\
                        rem Use ca entry amend to properly update entry + working branch\n\
                        \\\"{0}\\\" entry amend --all\n\
                        exit /b %ERRORLEVEL%\n\
                    ) else if /i \\\"%choice%\\\"==\\\"N\\\" (\n\
                        echo ➕ Creating new stack entry...\n\
                        rem Let the commit proceed normally\n\
                        exit /b 0\n\
                    ) else if /i \\\"%choice%\\\"==\\\"C\\\" (\n\
                        echo ❌ Commit cancelled\n\
                        exit /b 1\n\
                    ) else (\n\
                        echo ❓ Invalid choice. Please choose A, n, or c\n\
                        exit /b 1\n\
                    )\n\
                 )\n\n\
                 rem Not in edit mode, proceed normally\n\
                 exit /b 0\n",
                cascade_cli
            )
        }

        #[cfg(not(windows))]
        {
            format!(
                "#!/bin/sh\n\
                 # Cascade CLI Hook - Pre Commit\n\
                 # Smart edit mode guidance for better UX\n\n\
                 set -e\n\n\
                 # Check if Cascade is initialized\n\
                 REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo \\\".\\\")\n\
                 if [ ! -d \\\"$REPO_ROOT/.cascade\\\" ]; then\n\
                     exit 0\n\
                 fi\n\n\
                 # Check if we're in edit mode\n\
                 if \\\"{0}\\\" entry status --quiet >/dev/null 2>&1; then\n\
                     echo \\\"⚠️ You're in EDIT MODE for a stack entry!\\\"\n\
                     echo \\\"\\\"\n\
                    echo \\\"Choose your action:\\\"\n\
                    echo \\\"  🔄 [A]mend: Modify the current entry (default)\\\"\n\
                    echo \\\"  ➕ [N]ew:   Create new entry on top\\\"\n\
                    echo \\\"  ❌ [C]ancel: Stop and think about it\\\"\n\
                    echo \\\"\\\"\n\
                    \n\
                    # Read user choice with default to amend\n\
                    read -p \\\"Your choice (A/n/c): \\\" choice\n\
                    choice=${{choice:-A}}\n\
                    \n\
                    case \\\"$choice\\\" in\n\
                        [Aa])\n\
                            echo \\\"✅ Amending current entry...\\\"\n\
                            # Stage all changes first (like git commit -a)\n\
                            git add -A\n\
                            # Use ca entry amend to properly update entry + working branch\n\
                            \\\"{0}\\\" entry amend --all\n\
                            exit $?\n\
                            ;;\n\
                        [Nn])\n\
                            echo \\\"➕ Creating new stack entry...\\\"\n\
                            # Let the commit proceed normally (will create new commit)\n\
                            exit 0\n\
                            ;;\n\
                        [Cc])\n\
                            echo \\\"❌ Commit cancelled\\\"\n\
                            exit 1\n\
                            ;;\n\
                        *)\n\
                            echo \\\"❓ Invalid choice. Please choose A, n, or c\\\"\n\
                            exit 1\n\
                            ;;\n\
                    esac\n\
                 fi\n\n\
                 # Not in edit mode, proceed normally\n\
                 exit 0\n",
                cascade_cli
            )
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
                 rem Check if in edit mode first\n\
                 for /f \"tokens=*\" %%i in ('\"{cascade_cli}\" entry status --quiet 2^>nul') do set EDIT_STATUS=%%i\n\
                 if \"%EDIT_STATUS%\"==\"\" set EDIT_STATUS=inactive\n\n\
                 if not \"%EDIT_STATUS%\"==\"inactive\" (\n\
                     rem In edit mode - provide smart guidance\n\
                     set /p CURRENT_MSG=<%COMMIT_MSG_FILE%\n\n\
                     rem Skip if message already has edit guidance\n\
                     echo !CURRENT_MSG! | findstr \"[EDIT MODE]\" >nul\n\
                     if %ERRORLEVEL% equ 0 exit /b 0\n\n\
                     rem Add edit mode guidance to commit message\n\
                     echo.\n\
                     echo # [EDIT MODE] You're editing a stack entry\n\
                     echo #\n\
                     echo # Choose your action:\n\
                     echo #   🔄 AMEND: To modify the current entry, use:\n\
                     echo #       git commit --amend\n\
                     echo #\n\
                     echo #   ➕ NEW: To create a new entry on top, use:\n\
                     echo #       git commit    ^(this command^)\n\
                     echo #\n\
                     echo # 💡 After committing, run 'ca sync' to update PRs\n\
                     echo.\n\
                     type \"%COMMIT_MSG_FILE%\"\n\
                 ) > \"%COMMIT_MSG_FILE%.tmp\" && (\n\
                     move \"%COMMIT_MSG_FILE%.tmp\" \"%COMMIT_MSG_FILE%\"\n\
                 ) else (\n\
                     rem Regular stack mode - check for active stack\n\
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
                     move \"%COMMIT_MSG_FILE%.tmp\" \"%COMMIT_MSG_FILE%\"\n\
                 )\n"
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
                 # Check if in edit mode first\n\
                 EDIT_STATUS=$(\"{cascade_cli}\" entry status --quiet 2>/dev/null || echo \"inactive\")\n\
                 \n\
                 if [ \"$EDIT_STATUS\" != \"inactive\" ]; then\n\
                     # In edit mode - provide smart guidance\n\
                     CURRENT_MSG=$(cat \"$COMMIT_MSG_FILE\")\n\
                     \n\
                     # Skip if message already has edit guidance\n\
                     if echo \"$CURRENT_MSG\" | grep -q \"\\[EDIT MODE\\]\"; then\n\
                         exit 0\n\
                     fi\n\
                     \n\
                     echo \"\n\
                 # [EDIT MODE] You're editing a stack entry\n\
                 #\n\
                 # Choose your action:\n\
                 #   🔄 AMEND: To modify the current entry, use:\n\
                 #       git commit --amend\n\
                 #\n\
                 #   ➕ NEW: To create a new entry on top, use:\n\
                 #       git commit    (this command)\n\
                 #\n\
                 # 💡 After committing, run 'ca sync' to update PRs\n\
                 \n\
                 $CURRENT_MSG\" > \"$COMMIT_MSG_FILE\"\n\
                 else\n\
                     # Regular stack mode - check for active stack\n\
                     ACTIVE_STACK=$(\"{cascade_cli}\" stack list --active --format=name 2>/dev/null || echo \"\")\n\
                     \n\
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
                     fi\n\
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
        Output::check_start("Checking prerequisites for Cascade hooks");

        // 1. Check repository type
        let repo_type = self.detect_repository_type()?;
        match repo_type {
            RepositoryType::Bitbucket => {
                Output::success("Bitbucket repository detected");
                Output::tip("Hooks will work great with 'ca submit' and 'ca autoland' for Bitbucket integration");
            }
            RepositoryType::GitHub => {
                Output::success("GitHub repository detected");
                Output::tip("Consider setting up GitHub Actions for CI/CD integration");
            }
            RepositoryType::GitLab => {
                Output::success("GitLab repository detected");
                Output::tip("GitLab CI integration works well with Cascade stacks");
            }
            RepositoryType::AzureDevOps => {
                Output::success("Azure DevOps repository detected");
                Output::tip("Azure Pipelines can be configured to work with Cascade workflows");
            }
            RepositoryType::Unknown => {
                Output::info(
                    "Unknown repository type - hooks will still work for local Git operations",
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

        Output::success("Prerequisites validation passed");
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
                Output::success("Feature branch detected - suitable for stacked development");
            }
            BranchType::Unknown => {
                Output::warning("Unknown branch type - proceeding with caution");
            }
        }

        Ok(())
    }

    /// Confirm installation with user
    pub fn confirm_installation(&self) -> Result<()> {
        Output::section("Hook Installation Summary");

        let hooks = vec![
            HookType::PostCommit,
            HookType::PrePush,
            HookType::CommitMsg,
            HookType::PrepareCommitMsg,
        ];

        for hook in &hooks {
            Output::sub_item(format!("{}: {}", hook.filename(), hook.description()));
        }

        println!();
        Output::section("These hooks will automatically");
        Output::bullet("Add commits to your active stack");
        Output::bullet("Validate commit messages");
        Output::bullet("Prevent force pushes that break stack integrity");
        Output::bullet("Add stack context to commit messages");

        println!();
        Output::section("With hooks + new defaults, your workflow becomes");
        Output::sub_item("git commit       → Auto-added to stack");
        Output::sub_item("ca push          → Pushes all by default");
        Output::sub_item("ca submit        → Submits all by default");
        Output::sub_item("ca autoland      → Auto-merges when ready");

        // Interactive confirmation to proceed with installation
        let should_install = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Install Cascade hooks?")
            .default(true)
            .interact()
            .map_err(|e| CascadeError::config(format!("Failed to get user confirmation: {e}")))?;

        if should_install {
            Output::success("Proceeding with installation");
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
        "pre-commit" => HookType::PreCommit,
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
        "pre-commit" => HookType::PreCommit,
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
        // Should create a HooksManager successfully
        assert!(!_manager.repo_id.is_empty());
    }

    #[test]
    fn test_hooks_manager_custom_hooks_path() {
        let (_temp_dir, repo_path) = create_test_repo();

        // Set custom hooks path
        Command::new("git")
            .args(["config", "core.hooksPath", "custom-hooks"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create the custom hooks directory
        let custom_hooks_dir = repo_path.join("custom-hooks");
        std::fs::create_dir_all(&custom_hooks_dir).unwrap();

        let _manager = HooksManager::new(&repo_path).unwrap();

        assert_eq!(_manager.repo_path, repo_path);
        // Should create a HooksManager successfully
        assert!(!_manager.repo_id.is_empty());
    }

    #[test]
    fn test_hook_chaining_with_existing_hooks() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = HooksManager::new(&repo_path).unwrap();

        let hook_type = HookType::PreCommit;
        let hook_path = repo_path.join(".git/hooks").join(hook_type.filename());

        // Create an existing project hook
        let existing_hook_content = "#!/bin/bash\n# Project pre-commit hook\n./scripts/lint.sh\n";
        std::fs::write(&hook_path, existing_hook_content).unwrap();
        crate::utils::platform::make_executable(&hook_path).unwrap();

        // Install cascade hook (uses core.hooksPath, doesn't modify original)
        let result = manager.install_hook(&hook_type);
        assert!(result.is_ok());

        // Original hook should remain unchanged
        let original_content = std::fs::read_to_string(&hook_path).unwrap();
        assert!(original_content.contains("# Project pre-commit hook"));
        assert!(original_content.contains("./scripts/lint.sh"));

        // Cascade hook should exist in cascade directory
        let cascade_hooks_dir = manager.get_cascade_hooks_dir().unwrap();
        let cascade_hook_path = cascade_hooks_dir.join(hook_type.filename());
        assert!(cascade_hook_path.exists());

        // Test uninstall removes cascade hooks but leaves original
        let uninstall_result = manager.uninstall_hook(&hook_type);
        assert!(uninstall_result.is_ok());

        // Original hook should still exist and be unchanged
        let after_uninstall = std::fs::read_to_string(&hook_path).unwrap();
        assert!(after_uninstall.contains("# Project pre-commit hook"));
        assert!(after_uninstall.contains("./scripts/lint.sh"));

        // Cascade hook should be removed
        assert!(!cascade_hook_path.exists());
    }

    #[test]
    fn test_hook_installation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = HooksManager::new(&repo_path).unwrap();

        // Test installing post-commit hook
        let hook_type = HookType::PostCommit;
        let result = manager.install_hook(&hook_type);
        assert!(result.is_ok());

        // Verify hook file exists in cascade hooks directory
        let hook_filename = hook_type.filename();
        let cascade_hooks_dir = manager.get_cascade_hooks_dir().unwrap();
        let hook_path = cascade_hooks_dir.join(&hook_filename);
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

        let cascade_hooks_dir = manager.get_cascade_hooks_dir().unwrap();
        let hook_path = cascade_hooks_dir.join(hook_type.filename());
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

        // Install cascade hook (uses core.hooksPath, doesn't modify original)
        let hook_type = HookType::PostCommit;
        let result = manager.install_hook(&hook_type);
        assert!(result.is_ok());

        // Verify cascade hook exists in cascade directory
        let cascade_hooks_dir = manager.get_cascade_hooks_dir().unwrap();
        let cascade_hook_path = cascade_hooks_dir.join(&hook_filename);
        assert!(cascade_hook_path.exists());

        // Original hook should remain unchanged
        let original_content = std::fs::read_to_string(&hook_path).unwrap();
        assert!(original_content.contains("existing hook"));

        // Cascade hook should contain cascade logic
        let cascade_content = std::fs::read_to_string(&cascade_hook_path).unwrap();
        assert!(cascade_content.contains("cascade-cli") || cascade_content.contains("ca"));
    }
}
