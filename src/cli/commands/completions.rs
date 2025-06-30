use crate::cli::Cli;
use crate::errors::{CascadeError, Result};
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::fs;
use std::io;
use std::path::PathBuf;

/// Generate shell completions for the specified shell
pub fn generate_completions(shell: Shell) -> Result<()> {
    let mut cmd = Cli::command();
    let bin_name = "cc";

    generate(shell, &mut cmd, bin_name, &mut io::stdout());
    Ok(())
}

/// Install shell completions to the system
pub fn install_completions(shell: Option<Shell>) -> Result<()> {
    let shells_to_install = if let Some(shell) = shell {
        vec![shell]
    } else {
        // Auto-detect available shells
        detect_available_shells()
    };

    let mut installed = Vec::new();
    let mut errors = Vec::new();

    for shell in shells_to_install {
        match install_completion_for_shell(shell) {
            Ok(path) => {
                installed.push((shell, path));
            }
            Err(e) => {
                errors.push((shell, e));
            }
        }
    }

    // Report results
    if !installed.is_empty() {
        println!("✅ Shell completions installed:");
        for (shell, path) in installed {
            println!("   {:?}: {}", shell, path.display());
        }

        println!("\n💡 Next steps:");
        println!("   1. Restart your shell or run: source ~/.bashrc (or equivalent)");
        println!("   2. Try: cc <TAB><TAB>");
    }

    if !errors.is_empty() {
        println!("\n⚠️  Some installations failed:");
        for (shell, error) in errors {
            println!("   {shell:?}: {error}");
        }
    }

    Ok(())
}

/// Detect which shells are available on the system
fn detect_available_shells() -> Vec<Shell> {
    let mut shells = Vec::new();

    // Check for bash
    if which_shell("bash").is_some() {
        shells.push(Shell::Bash);
    }

    // Check for zsh
    if which_shell("zsh").is_some() {
        shells.push(Shell::Zsh);
    }

    // Check for fish
    if which_shell("fish").is_some() {
        shells.push(Shell::Fish);
    }

    // Default to bash if nothing found
    if shells.is_empty() {
        shells.push(Shell::Bash);
    }

    shells
}

/// Check if a shell exists in PATH
fn which_shell(shell: &str) -> Option<PathBuf> {
    std::env::var("PATH")
        .ok()?
        .split(crate::utils::platform::path_separator())
        .map(PathBuf::from)
        .find_map(|path| {
            let shell_path = path.join(crate::utils::platform::executable_name(shell));
            if crate::utils::platform::is_executable(&shell_path) {
                Some(shell_path)
            } else {
                None
            }
        })
}

/// Install completion for a specific shell
fn install_completion_for_shell(shell: Shell) -> Result<PathBuf> {
    // Get platform-specific completion directories
    let completion_dirs = crate::utils::platform::shell_completion_dirs();

    let (completion_dir, filename) = match shell {
        Shell::Bash => {
            // Find the first suitable bash completion directory
            let bash_dirs: Vec<_> = completion_dirs
                .iter()
                .filter(|(name, _)| name.contains("bash"))
                .map(|(_, path)| path.clone())
                .collect();

            let dir = bash_dirs
                .into_iter()
                .find(|d| d.exists() || d.parent().is_some_and(|p| p.exists()))
                .or_else(|| {
                    // Fallback to user-specific directory
                    dirs::home_dir().map(|h| h.join(".bash_completion.d"))
                })
                .ok_or_else(|| {
                    CascadeError::config("Could not find suitable bash completion directory")
                })?;

            (dir, "cc")
        }
        Shell::Zsh => {
            // Find the first suitable zsh completion directory
            let zsh_dirs: Vec<_> = completion_dirs
                .iter()
                .filter(|(name, _)| name.contains("zsh"))
                .map(|(_, path)| path.clone())
                .collect();

            let dir = zsh_dirs
                .into_iter()
                .find(|d| d.exists() || d.parent().is_some_and(|p| p.exists()))
                .or_else(|| {
                    // Fallback to user-specific directory
                    dirs::home_dir().map(|h| h.join(".zsh/completions"))
                })
                .ok_or_else(|| {
                    CascadeError::config("Could not find suitable zsh completion directory")
                })?;

            (dir, "_cc")
        }
        Shell::Fish => {
            // Find the first suitable fish completion directory
            let fish_dirs: Vec<_> = completion_dirs
                .iter()
                .filter(|(name, _)| name.contains("fish"))
                .map(|(_, path)| path.clone())
                .collect();

            let dir = fish_dirs
                .into_iter()
                .find(|d| d.exists() || d.parent().is_some_and(|p| p.exists()))
                .or_else(|| {
                    // Fallback to user-specific directory
                    dirs::home_dir().map(|h| h.join(".config/fish/completions"))
                })
                .ok_or_else(|| {
                    CascadeError::config("Could not find suitable fish completion directory")
                })?;

            (dir, "cc.fish")
        }
        _ => {
            return Err(CascadeError::config(format!(
                "Unsupported shell: {shell:?}"
            )));
        }
    };

    // Create directory if it doesn't exist
    if !completion_dir.exists() {
        fs::create_dir_all(&completion_dir)?;
    }

    let completion_file =
        completion_dir.join(crate::utils::path_validation::sanitize_filename(filename));

    // Validate the completion file path for security
    crate::utils::path_validation::validate_config_path(&completion_file, &completion_dir)?;

    // Generate completion content
    let mut cmd = Cli::command();
    let mut content = Vec::new();
    generate(shell, &mut cmd, "cc", &mut content);

    // Write to file atomically
    crate::utils::atomic_file::write_bytes(&completion_file, &content)?;

    Ok(completion_file)
}

/// Show installation status and guidance
pub fn show_completions_status() -> Result<()> {
    println!("🚀 Shell Completions Status");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let available_shells = detect_available_shells();

    println!("\n📊 Available shells:");
    for shell in &available_shells {
        let status = check_completion_installed(*shell);
        let status_icon = if status { "✅" } else { "❌" };
        println!("   {status_icon} {shell:?}");
    }

    if available_shells
        .iter()
        .any(|s| !check_completion_installed(*s))
    {
        println!("\n💡 To install completions:");
        println!("   cc completions install");
        println!("   cc completions install --shell bash  # for specific shell");
    } else {
        println!("\n🎉 All available shells have completions installed!");
    }

    println!("\n🔧 Manual installation:");
    println!("   cc completions generate bash > ~/.bash_completion.d/cc");
    println!("   cc completions generate zsh > ~/.zsh/completions/_cc");
    println!("   cc completions generate fish > ~/.config/fish/completions/cc.fish");

    Ok(())
}

/// Check if completion is installed for a shell
fn check_completion_installed(shell: Shell) -> bool {
    let home_dir = match dirs::home_dir() {
        Some(dir) => dir,
        None => return false,
    };

    let possible_paths = match shell {
        Shell::Bash => vec![
            home_dir.join(".bash_completion.d/cc"),
            PathBuf::from("/usr/local/etc/bash_completion.d/cc"),
            PathBuf::from("/etc/bash_completion.d/cc"),
        ],
        Shell::Zsh => vec![
            home_dir.join(".oh-my-zsh/completions/_cc"),
            home_dir.join(".zsh/completions/_cc"),
            PathBuf::from("/usr/local/share/zsh/site-functions/_cc"),
        ],
        Shell::Fish => vec![home_dir.join(".config/fish/completions/cc.fish")],
        _ => return false,
    };

    possible_paths.iter().any(|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shells() {
        let shells = detect_available_shells();
        // Should always have at least one shell (bash fallback)
        assert!(!shells.is_empty());
    }

    #[test]
    fn test_generate_bash_completion() {
        // Just test that it doesn't panic
        let result = generate_completions(Shell::Bash);
        assert!(result.is_ok());
    }
}
