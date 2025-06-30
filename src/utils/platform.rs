use std::path::{Path, PathBuf};

/// Platform-specific utilities for handling cross-platform differences
///
/// This module centralizes platform detection and platform-specific behavior
/// to ensure consistent handling across Windows, macOS, and Linux.
///
/// Get the appropriate PATH environment variable separator for the current platform
pub fn path_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

/// Get the executable file extension for the current platform
pub fn executable_extension() -> &'static str {
    if cfg!(windows) {
        ".exe"
    } else {
        ""
    }
}

/// Add the appropriate executable extension to a binary name
pub fn executable_name(name: &str) -> String {
    format!("{}{}", name, executable_extension())
}

/// Check if a file is executable on the current platform
pub fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            // Check if any execute bit is set (owner, group, or other)
            metadata.permissions().mode() & 0o111 != 0
        } else {
            false
        }
    }

    #[cfg(windows)]
    {
        // On Windows, check if file exists and has executable extension
        if !path.exists() {
            return false;
        }

        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            matches!(ext.as_str(), "exe" | "bat" | "cmd" | "com" | "scr" | "ps1")
        } else {
            false
        }
    }
}

/// Make a file executable on the current platform
#[cfg(unix)]
pub fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = std::fs::metadata(path)?.permissions();
    // Set executable for owner, group, and others (if they have read permission)
    let current_mode = perms.mode();
    let new_mode = current_mode | ((current_mode & 0o444) >> 2);
    perms.set_mode(new_mode);
    std::fs::set_permissions(path, perms)
}

#[cfg(windows)]
pub fn make_executable(_path: &Path) -> std::io::Result<()> {
    // On Windows, executability is determined by file extension, not permissions
    // If we need to make something executable, we should ensure it has the right extension
    Ok(())
}

/// Get platform-specific shell completion directories
pub fn shell_completion_dirs() -> Vec<(String, PathBuf)> {
    let mut dirs = Vec::new();

    #[cfg(unix)]
    {
        // Standard Unix completion directories
        if let Some(home) = dirs::home_dir() {
            // User-specific directories
            dirs.push(("bash (user)".to_string(), home.join(".bash_completion.d")));
            dirs.push(("zsh (user)".to_string(), home.join(".zsh/completions")));
            dirs.push((
                "fish (user)".to_string(),
                home.join(".config/fish/completions"),
            ));
        }

        // System-wide directories (may require sudo)
        dirs.push((
            "bash (system)".to_string(),
            PathBuf::from("/usr/local/etc/bash_completion.d"),
        ));
        dirs.push((
            "bash (system alt)".to_string(),
            PathBuf::from("/etc/bash_completion.d"),
        ));
        dirs.push((
            "zsh (system)".to_string(),
            PathBuf::from("/usr/local/share/zsh/site-functions"),
        ));
        dirs.push((
            "zsh (system alt)".to_string(),
            PathBuf::from("/usr/share/zsh/site-functions"),
        ));
        dirs.push((
            "fish (system)".to_string(),
            PathBuf::from("/usr/local/share/fish/completions"),
        ));
        dirs.push((
            "fish (system alt)".to_string(),
            PathBuf::from("/usr/share/fish/completions"),
        ));
    }

    #[cfg(windows)]
    {
        // Windows-specific completion directories
        if let Some(home) = dirs::home_dir() {
            // PowerShell profile directory
            let ps_profile_dir = home
                .join("Documents")
                .join("WindowsPowerShell")
                .join("Modules");
            dirs.push(("PowerShell (user)".to_string(), ps_profile_dir));

            // Git Bash (if installed)
            let git_bash_completion = home
                .join("AppData")
                .join("Local")
                .join("Programs")
                .join("Git")
                .join("etc")
                .join("bash_completion.d");
            dirs.push(("Git Bash (user)".to_string(), git_bash_completion));
        }

        // System-wide directories
        if let Ok(program_files) = std::env::var("PROGRAMFILES") {
            let git_bash_system = PathBuf::from(program_files)
                .join("Git")
                .join("etc")
                .join("bash_completion.d");
            dirs.push(("Git Bash (system)".to_string(), git_bash_system));
        }
    }

    dirs
}

/// Get platform-specific Git hook script extension
pub fn git_hook_extension() -> &'static str {
    if cfg!(windows) {
        ".bat"
    } else {
        ""
    }
}

/// Create platform-specific Git hook content
pub fn create_git_hook_content(hook_name: &str, command: &str) -> String {
    #[cfg(windows)]
    {
        format!(
            "@echo off\n\
             rem Cascade CLI Git Hook: {hook_name}\n\
             rem Generated automatically - do not edit manually\n\n\
             \"{command}\" %*\n\
             if %ERRORLEVEL% neq 0 exit /b %ERRORLEVEL%\n"
        )
    }

    #[cfg(not(windows))]
    {
        format!(
            "#!/bin/sh\n\
             # Cascade CLI Git Hook: {hook_name}\n\
             # Generated automatically - do not edit manually\n\n\
             exec \"{command}\" \"$@\"\n"
        )
    }
}

/// Get the default shell for the current platform
pub fn default_shell() -> Option<String> {
    #[cfg(windows)]
    {
        // On Windows, prefer PowerShell, then Command Prompt
        if which_shell("powershell").is_some() {
            Some("powershell".to_string())
        } else if which_shell("cmd").is_some() {
            Some("cmd".to_string())
        } else {
            None
        }
    }

    #[cfg(not(windows))]
    {
        // On Unix, check common shells in order of preference
        for shell in &["zsh", "bash", "fish", "sh"] {
            if which_shell(shell).is_some() {
                return Some(shell.to_string());
            }
        }
        None
    }
}

/// Find a shell executable in PATH
fn which_shell(shell_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    let executable_name = executable_name(shell_name);

    for path_dir in path_var.split(path_separator()) {
        let shell_path = PathBuf::from(path_dir).join(&executable_name);
        if is_executable(&shell_path) {
            return Some(shell_path);
        }
    }
    None
}

/// Get platform-specific temporary directory with proper permissions
pub fn secure_temp_dir() -> std::io::Result<PathBuf> {
    let temp_dir = std::env::temp_dir();

    #[cfg(unix)]
    {
        // On Unix, ensure temp directory has proper permissions (700 = rwx------)
        use std::os::unix::fs::PermissionsExt;
        let temp_subdir = temp_dir.join(format!("cascade-{}", std::process::id()));
        std::fs::create_dir_all(&temp_subdir)?;

        let mut perms = std::fs::metadata(&temp_subdir)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&temp_subdir, perms)?;
        Ok(temp_subdir)
    }

    #[cfg(windows)]
    {
        // On Windows, create subdirectory but permissions are handled by ACLs
        let temp_subdir = temp_dir.join(format!("cascade-{}", std::process::id()));
        std::fs::create_dir_all(&temp_subdir)?;
        Ok(temp_subdir)
    }
}

/// Platform-specific line ending normalization
pub fn normalize_line_endings(content: &str) -> String {
    // Always normalize to Unix line endings internally
    // Git will handle conversion based on core.autocrlf setting
    content.replace("\r\n", "\n").replace('\r', "\n")
}

/// Get platform-specific environment variable for editor
pub fn default_editor() -> Option<String> {
    // Check common editor environment variables in order of preference
    for var in &["EDITOR", "VISUAL"] {
        if let Ok(editor) = std::env::var(var) {
            if !editor.trim().is_empty() {
                return Some(editor);
            }
        }
    }

    // Platform-specific defaults
    #[cfg(windows)]
    {
        // On Windows, try notepad as last resort
        Some("notepad".to_string())
    }

    #[cfg(not(windows))]
    {
        // On Unix, try common editors
        for editor in &["nano", "vim", "vi"] {
            if which_shell(editor).is_some() {
                return Some(editor.to_string());
            }
        }
        Some("vi".to_string()) // vi should always be available on Unix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_separator() {
        let separator = path_separator();
        if cfg!(windows) {
            assert_eq!(separator, ";");
        } else {
            assert_eq!(separator, ":");
        }
    }

    #[test]
    fn test_executable_extension() {
        let ext = executable_extension();
        if cfg!(windows) {
            assert_eq!(ext, ".exe");
        } else {
            assert_eq!(ext, "");
        }
    }

    #[test]
    fn test_executable_name() {
        let name = executable_name("test");
        if cfg!(windows) {
            assert_eq!(name, "test.exe");
        } else {
            assert_eq!(name, "test");
        }
    }

    #[test]
    fn test_git_hook_extension() {
        let ext = git_hook_extension();
        if cfg!(windows) {
            assert_eq!(ext, ".bat");
        } else {
            assert_eq!(ext, "");
        }
    }

    #[test]
    fn test_line_ending_normalization() {
        assert_eq!(normalize_line_endings("hello\r\nworld"), "hello\nworld");
        assert_eq!(normalize_line_endings("hello\rworld"), "hello\nworld");
        assert_eq!(normalize_line_endings("hello\nworld"), "hello\nworld");
        assert_eq!(normalize_line_endings("hello world"), "hello world");
    }

    #[test]
    fn test_shell_completion_dirs() {
        let dirs = shell_completion_dirs();
        assert!(
            !dirs.is_empty(),
            "Should return at least one completion directory"
        );

        // All paths should be absolute
        for (name, path) in &dirs {
            assert!(
                path.is_absolute(),
                "Completion directory should be absolute: {name} -> {path:?}"
            );
        }
    }

    #[test]
    fn test_default_shell() {
        // Should return some shell on any platform
        let shell = default_shell();
        if cfg!(windows) {
            // On Windows, should prefer PowerShell or cmd
            if let Some(shell_name) = shell {
                assert!(shell_name == "powershell" || shell_name == "cmd");
            }
        } else {
            // On Unix, should return a common shell
            if let Some(shell_name) = shell {
                assert!(["zsh", "bash", "fish", "sh"].contains(&shell_name.as_str()));
            }
        }
    }

    #[test]
    fn test_git_hook_content() {
        let content = create_git_hook_content("pre-commit", "/usr/bin/cc");

        if cfg!(windows) {
            assert!(content.contains("@echo off"));
            assert!(content.contains(".bat"));
            assert!(content.contains("ERRORLEVEL"));
        } else {
            assert!(content.starts_with("#!/bin/sh"));
            assert!(content.contains("exec"));
        }
    }
}
