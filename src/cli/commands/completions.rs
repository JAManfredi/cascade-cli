use crate::cli::output::Output;
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
    let bin_name = "ca";

    generate(shell, &mut cmd, bin_name, &mut io::stdout());
    Ok(())
}

/// Install shell completions to the system
pub fn install_completions(shell: Option<Shell>) -> Result<()> {
    let shells_to_install = if let Some(shell) = shell {
        vec![shell]
    } else {
        // Detect current shell first, then fall back to available shells
        detect_current_and_available_shells()
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
        Output::success("Shell completions installed:");
        for (shell, path) in &installed {
            Output::sub_item(format!("{:?}: {}", shell, path.display()));
        }

        println!();
        Output::tip("Next steps:");

        // Provide shell-specific setup instructions
        for (shell, path) in &installed {
            match shell {
                Shell::Zsh => {
                    if path.to_string_lossy().contains(".zsh/completions") {
                        println!();
                        Output::warning("⚠️  Zsh requires additional setup:");
                        Output::bullet("Add this to your ~/.zshrc:");
                        println!("      fpath=(~/.zsh/completions $fpath)");
                        println!("      autoload -Uz compinit && compinit");
                        Output::bullet("Then reload: source ~/.zshrc");
                    }
                }
                Shell::Bash => {
                    if path.to_string_lossy().contains(".bash_completion.d") {
                        println!();
                        Output::info("For bash completions to work:");
                        Output::bullet("Ensure bash-completion is installed");
                        Output::bullet("Then reload: source ~/.bashrc");
                    }
                }
                _ => {}
            }
        }

        println!();
        Output::bullet("Try: ca <TAB><TAB>");
    }

    if !errors.is_empty() {
        println!();
        Output::warning("Some installations failed:");
        for (shell, error) in errors {
            Output::sub_item(format!("{shell:?}: {error}"));
        }
    }

    Ok(())
}

/// Detect current shell first, then fall back to available shells
fn detect_current_and_available_shells() -> Vec<Shell> {
    let mut shells = Vec::new();

    // First, try to detect the current shell from SHELL environment variable
    if let Some(current_shell) = detect_current_shell() {
        shells.push(current_shell);
        Output::info(format!("Detected current shell: {current_shell:?}"));
        return shells; // Only install for current shell
    }

    // Fall back to detecting all available shells
    Output::info("Could not detect current shell, checking available shells...");
    detect_available_shells()
}

/// Detect the current shell from the SHELL environment variable
fn detect_current_shell() -> Option<Shell> {
    let shell_path = std::env::var("SHELL").ok()?;
    let shell_name = std::path::Path::new(&shell_path).file_name()?.to_str()?;

    match shell_name {
        "bash" => Some(Shell::Bash),
        "zsh" => Some(Shell::Zsh),
        "fish" => Some(Shell::Fish),
        _ => None,
    }
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
            // Prioritize user directories over system directories
            let bash_dirs: Vec<_> = completion_dirs
                .iter()
                .filter(|(name, _)| name.contains("bash"))
                .collect();

            // First try user directories
            let user_dir = bash_dirs
                .iter()
                .find(|(name, _)| name.contains("user"))
                .map(|(_, path)| path.clone())
                .filter(|d| d.exists() || d.parent().is_some_and(|p| p.exists()));

            // If no user directory works, try system directories
            let system_dir = if user_dir.is_none() {
                bash_dirs
                    .iter()
                    .find(|(name, _)| name.contains("system"))
                    .map(|(_, path)| path.clone())
                    .filter(|d| d.exists() || d.parent().is_some_and(|p| p.exists()))
            } else {
                None
            };

            let dir = user_dir
                .or(system_dir)
                .or_else(|| {
                    // Fallback to user-specific directory
                    dirs::home_dir().map(|h| h.join(".bash_completion.d"))
                })
                .ok_or_else(|| {
                    CascadeError::config("Could not find suitable bash completion directory")
                })?;

            (dir, "ca")
        }
        Shell::Zsh => {
            // Prioritize user directories over system directories
            let zsh_dirs: Vec<_> = completion_dirs
                .iter()
                .filter(|(name, _)| name.contains("zsh"))
                .collect();

            // First try user directories
            let user_dir = zsh_dirs
                .iter()
                .find(|(name, _)| name.contains("user"))
                .map(|(_, path)| path.clone())
                .filter(|d| d.exists() || d.parent().is_some_and(|p| p.exists()));

            // If no user directory works, try system directories
            let system_dir = if user_dir.is_none() {
                zsh_dirs
                    .iter()
                    .find(|(name, _)| name.contains("system"))
                    .map(|(_, path)| path.clone())
                    .filter(|d| d.exists() || d.parent().is_some_and(|p| p.exists()))
            } else {
                None
            };

            let dir = user_dir
                .or(system_dir)
                .or_else(|| {
                    // Fallback to user-specific directory
                    dirs::home_dir().map(|h| h.join(".zsh/completions"))
                })
                .ok_or_else(|| {
                    CascadeError::config("Could not find suitable zsh completion directory")
                })?;

            (dir, "_ca")
        }
        Shell::Fish => {
            // Prioritize user directories over system directories
            let fish_dirs: Vec<_> = completion_dirs
                .iter()
                .filter(|(name, _)| name.contains("fish"))
                .collect();

            // First try user directories
            let user_dir = fish_dirs
                .iter()
                .find(|(name, _)| name.contains("user"))
                .map(|(_, path)| path.clone())
                .filter(|d| d.exists() || d.parent().is_some_and(|p| p.exists()));

            // If no user directory works, try system directories
            let system_dir = if user_dir.is_none() {
                fish_dirs
                    .iter()
                    .find(|(name, _)| name.contains("system"))
                    .map(|(_, path)| path.clone())
                    .filter(|d| d.exists() || d.parent().is_some_and(|p| p.exists()))
            } else {
                None
            };

            let dir = user_dir
                .or(system_dir)
                .or_else(|| {
                    // Fallback to user-specific directory
                    dirs::home_dir().map(|h| h.join(".config/fish/completions"))
                })
                .ok_or_else(|| {
                    CascadeError::config("Could not find suitable fish completion directory")
                })?;

            (dir, "ca.fish")
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
    generate(shell, &mut cmd, "ca", &mut content);

    // Add custom completion logic for stack names
    let custom_completion = generate_custom_completion(shell);
    if !custom_completion.is_empty() {
        content.extend_from_slice(custom_completion.as_bytes());
    }

    // Write to file atomically, with fallback for lock failures
    match crate::utils::atomic_file::write_bytes(&completion_file, &content) {
        Ok(()) => {}
        Err(e) if e.to_string().contains("Timeout waiting for lock") => {
            // Lock failure - try without locking for user directories
            if completion_dir.to_string_lossy().contains(
                &dirs::home_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            ) {
                // This is a user directory, try direct write
                std::fs::write(&completion_file, &content)?;
            } else {
                // System directory, propagate the error
                return Err(e);
            }
        }
        Err(e) => return Err(e),
    }

    Ok(completion_file)
}

/// Show installation status and guidance
pub fn show_completions_status() -> Result<()> {
    Output::section("Shell Completions Status");

    let available_shells = detect_available_shells();

    Output::section("Available shells");
    for shell in &available_shells {
        let status = check_completion_installed(*shell);
        if status {
            Output::success(format!("{shell:?}"));
        } else {
            Output::error(format!("{shell:?}"));
        }
    }

    let all_installed = available_shells
        .iter()
        .all(|s| check_completion_installed(*s));

    if !all_installed {
        println!();
        Output::tip("To install completions:");
        Output::command_example("ca completions install");
        Output::command_example("ca completions install --shell bash  # for specific shell");
    } else {
        println!();
        Output::success("All available shells have completions installed!");
        
        // Check if zsh is available and provide setup instructions
        if available_shells.contains(&Shell::Zsh) {
            println!();
            
            // Check if zsh is already configured
            let zshrc_path = dirs::home_dir()
                .map(|h| h.join(".zshrc"))
                .unwrap_or_else(|| PathBuf::from("~/.zshrc"));
            
            let mut needs_fpath = true;
            let mut needs_compinit = true;
            
            let mut using_omz = false;
            let mut omz_line = None;
            
            if let Ok(zshrc_content) = std::fs::read_to_string(&zshrc_path) {
                // Check if Oh-My-Zsh is being used
                if zshrc_content.contains("oh-my-zsh.sh") {
                    using_omz = true;
                    // Find the line number where Oh-My-Zsh is sourced
                    for (i, line) in zshrc_content.lines().enumerate() {
                        if line.contains("source") && line.contains("oh-my-zsh.sh") {
                            omz_line = Some(i + 1);
                            break;
                        }
                    }
                }
                
                if zshrc_content.contains("fpath=(~/.zsh/completions") 
                    || zshrc_content.contains("fpath=(\"$HOME/.zsh/completions\"")
                    || zshrc_content.contains("fpath=($HOME/.zsh/completions") {
                    needs_fpath = false;
                }
                if zshrc_content.contains("compinit") {
                    needs_compinit = false;
                }
            }
            
            if needs_fpath || needs_compinit {
                Output::warning("Zsh requires additional setup for completions to work");
                println!();
                
                if using_omz {
                    Output::sub_item("Detected Oh-My-Zsh - special setup required:");
                    println!();
                    if let Some(line_num) = omz_line {
                        Output::info(format!("Oh-My-Zsh loads at line {} in ~/.zshrc", line_num));
                        Output::sub_item("The fpath MUST be set BEFORE Oh-My-Zsh loads");
                        Output::sub_item("Oh-My-Zsh calls compinit internally, so DON'T add compinit yourself");
                        println!();
                    }
                    
                    Output::sub_item("Option 1: Manual edit (recommended)");
                    Output::bullet("Open ~/.zshrc in an editor");
                    Output::bullet("Find the line: source $ZSH/oh-my-zsh.sh");
                    Output::bullet("Add this line BEFORE it:");
                    println!("      fpath=(~/.zsh/completions $fpath)");
                    Output::bullet("Make sure there's NO 'compinit' line at the end of ~/.zshrc");
                    Output::bullet("Save, then clear Oh-My-Zsh cache and reload:");
                    println!("      rm -f ~/.zcompdump && exec zsh");
                    println!();
                    
                    Output::sub_item("Option 2: Automatic (requires sed)");
                    if let Some(line_num) = omz_line {
                        let insert_line = line_num;
                        Output::command_example(&format!(
                            "sed -i.bak '{}i\\fpath=(~/.zsh/completions $fpath)' ~/.zshrc",
                            insert_line
                        ));
                        Output::command_example("rm -f ~/.zcompdump && exec zsh");
                    }
                } else {
                    Output::sub_item("Run these commands to complete setup:");
                    println!();
                    
                    if needs_fpath {
                        Output::command_example(r#"echo 'fpath=(~/.zsh/completions $fpath)' >> ~/.zshrc"#);
                    }
                    if needs_compinit {
                        Output::command_example(r#"echo 'autoload -Uz compinit && compinit' >> ~/.zshrc"#);
                    }
                    Output::command_example("source ~/.zshrc");
                }
            } else {
                Output::success("Zsh is properly configured for completions!");
                
                if using_omz {
                    println!();
                    Output::tip("If completions aren't working, clear Oh-My-Zsh cache:");
                    Output::command_example("rm -f ~/.zcompdump && exec zsh");
                }
            }
        }
    }

    println!();
    Output::section("Manual installation");
    Output::command_example("ca completions generate bash > ~/.bash_completion.d/ca");
    Output::command_example("ca completions generate zsh > ~/.zsh/completions/_ca");
    Output::command_example("ca completions generate fish > ~/.config/fish/completions/ca.fish");

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
            home_dir.join(".bash_completion.d/ca"),
            PathBuf::from("/usr/local/etc/bash_completion.d/ca"),
            PathBuf::from("/etc/bash_completion.d/ca"),
        ],
        Shell::Zsh => vec![
            home_dir.join(".oh-my-zsh/completions/_ca"),
            home_dir.join(".zsh/completions/_ca"),
            PathBuf::from("/usr/local/share/zsh/site-functions/_ca"),
        ],
        Shell::Fish => vec![home_dir.join(".config/fish/completions/ca.fish")],
        _ => return false,
    };

    possible_paths.iter().any(|path| path.exists())
}

/// Generate custom completion logic for dynamic values
fn generate_custom_completion(shell: Shell) -> String {
    match shell {
        Shell::Bash => {
            r#"
# Custom completion for ca switch command
_ca_switch_completion() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local stacks=$(ca completion-helper stack-names 2>/dev/null)
    COMPREPLY=($(compgen -W "$stacks" -- "$cur"))
}

# Replace the default completion for 'ca switch' with our custom function
complete -F _ca_switch_completion ca
"#.to_string()
        }
        Shell::Zsh => {
            r#"
# Custom completion for ca switch command
_ca_switch_completion() {
    local stacks=($(ca completion-helper stack-names 2>/dev/null))
    _describe 'stacks' stacks
}

# Override the switch completion
compdef _ca_switch_completion ca switch
"#.to_string()
        }
        Shell::Fish => {
            r#"
# Custom completion for ca switch command
complete -c ca -f -n '__fish_seen_subcommand_from switch' -a '(ca completion-helper stack-names 2>/dev/null)'
"#.to_string()
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shells() {
        let shells = detect_available_shells();
        assert!(!shells.is_empty());
    }

    #[test]
    fn test_generate_bash_completion() {
        let result = generate_completions(Shell::Bash);
        assert!(result.is_ok());
    }

    #[test]
    fn test_detect_current_shell() {
        // Test with a mocked SHELL environment variable
        std::env::set_var("SHELL", "/bin/zsh");
        let shell = detect_current_shell();
        assert_eq!(shell, Some(Shell::Zsh));

        std::env::set_var("SHELL", "/usr/bin/bash");
        let shell = detect_current_shell();
        assert_eq!(shell, Some(Shell::Bash));

        std::env::set_var("SHELL", "/usr/local/bin/fish");
        let shell = detect_current_shell();
        assert_eq!(shell, Some(Shell::Fish));

        std::env::set_var("SHELL", "/bin/unknown");
        let shell = detect_current_shell();
        assert_eq!(shell, None);

        // Clean up
        std::env::remove_var("SHELL");
    }
}
