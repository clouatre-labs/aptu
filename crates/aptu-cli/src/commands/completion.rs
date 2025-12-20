//! Shell completion generation and installation.

use anyhow::{Context, Result, anyhow};
use clap::CommandFactory;
use clap_complete::Shell;
use clap_complete::generate;
use console::style;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tracing::debug;

use crate::cli::Cli;

/// Configuration for shell-specific completion paths and instructions.
#[derive(Debug, Clone)]
pub struct ShellConfig {
    /// Path where completion script should be installed
    pub completion_path: PathBuf,
    /// Configuration instructions for the shell
    pub config_instructions: String,
}

impl ShellConfig {
    /// Create a new `ShellConfig` for the given shell.
    pub fn new(shell: Shell) -> Result<Self> {
        let completion_path = get_completion_path(shell)?;
        let config_instructions = get_config_instructions(shell);

        Ok(Self {
            completion_path,
            config_instructions,
        })
    }
}

/// Determine the completion file path for a given shell.
///
/// Returns the standard location where shell completions should be installed.
fn get_completion_path(shell: Shell) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;

    let path = match shell {
        Shell::Bash => home.join(".bash_completion.d/aptu"),
        Shell::Zsh => home.join(".zsh/completions/_aptu"),
        Shell::Fish => home.join(".config/fish/completions/aptu.fish"),
        Shell::PowerShell => {
            // PowerShell uses a different approach - we'll return a placeholder
            // since it requires profile modification
            home.join(".config/powershell/profile.ps1")
        }
        Shell::Elvish => home.join(".local/share/elvish/lib/aptu.elv"),
        _ => {
            return Err(anyhow!(
                "Unsupported shell: {shell:?}. Supported shells: bash, zsh, fish, powershell, elvish"
            ));
        }
    };

    Ok(path)
}

/// Get shell-specific configuration instructions.
fn get_config_instructions(shell: Shell) -> String {
    match shell {
        Shell::Bash => "Add to ~/.bashrc or ~/.bash_profile:\n  \
             source ~/.bash_completion.d/aptu"
            .to_string(),
        Shell::Zsh => "Add to ~/.zshrc (before compinit):\n  \
             fpath=(~/.zsh/completions $fpath)\n  \
             autoload -U compinit && compinit -i"
            .to_string(),
        Shell::Fish => "Completions are automatically loaded from ~/.config/fish/completions/\n  \
             No additional configuration needed."
            .to_string(),
        Shell::PowerShell => "Add to your PowerShell profile ($PROFILE):\n  \
             . $HOME/.config/powershell/profile.ps1"
            .to_string(),
        Shell::Elvish => "Add to ~/.config/elvish/rc.elv:\n  \
             use aptu"
            .to_string(),
        _ => "Manual configuration required.".to_string(),
    }
}

/// Detect the current shell from environment.
///
/// Checks $SHELL environment variable and returns the corresponding Shell type.
fn detect_shell() -> Result<Shell> {
    let shell_env = std::env::var("SHELL")
        .context("$SHELL environment variable not set. Use --shell to specify.")?;

    let shell_name = std::path::Path::new(&shell_env)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Could not parse shell name from $SHELL"))?;

    match shell_name {
        "bash" => Ok(Shell::Bash),
        "zsh" => Ok(Shell::Zsh),
        "fish" => Ok(Shell::Fish),
        "pwsh" | "powershell" => Ok(Shell::PowerShell),
        "elvish" => Ok(Shell::Elvish),
        _ => Err(anyhow!(
            "Unsupported shell: {shell_name}. Supported: bash, zsh, fish, powershell, elvish"
        )),
    }
}

/// Generate completion script to stdout.
pub fn run_generate(shell: Shell) -> Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut std::io::stdout());
    std::io::stdout().flush()?;
    Ok(())
}

/// Install completion script to the standard location.
///
/// Auto-detects shell from $SHELL, creates parent directories if needed,
/// and writes the completion script. Prints configuration instructions.
pub fn run_install(shell: Option<Shell>, dry_run: bool) -> Result<()> {
    // Determine shell: explicit flag > auto-detect
    let shell = match shell {
        Some(s) => s,
        None => detect_shell()?,
    };

    let config = ShellConfig::new(shell)?;

    if dry_run {
        println!(
            "{}",
            style("DRY RUN - No files will be modified").yellow().bold()
        );
        println!();
        println!("{}", style(format!("Shell: {shell:?}")).cyan());
        println!(
            "{}",
            style(format!(
                "Completion path: {}",
                config.completion_path.display()
            ))
            .cyan()
        );
        println!();
        println!("{}", style("Configuration instructions:").bold());
        println!("{}", config.config_instructions);
        println!();
        return Ok(());
    }

    // Create parent directories if needed
    if let Some(parent) = config.completion_path.parent()
        && !parent.exists()
    {
        debug!("Creating parent directory: {}", parent.display());
        fs::create_dir_all(parent)
            .context(format!("Failed to create directory: {}", parent.display()))?;
    }

    // Generate completion script to a string
    let mut completion_script = Vec::new();
    {
        let mut cmd = Cli::command();
        let name = cmd.get_name().to_string();
        generate(shell, &mut cmd, name, &mut completion_script);
    }

    // Write to file
    debug!(
        "Writing completion script to: {}",
        config.completion_path.display()
    );
    let mut file = fs::File::create(&config.completion_path).context(format!(
        "Failed to create file: {}",
        config.completion_path.display()
    ))?;
    file.write_all(&completion_script)
        .context("Failed to write completion script")?;

    // Set appropriate permissions (readable by user)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o644);
        fs::set_permissions(&config.completion_path, perms)
            .context("Failed to set file permissions")?;
    }

    // Print success message
    println!();
    println!(
        "{}",
        style("Completion script installed successfully!")
            .green()
            .bold()
    );
    println!(
        "  {}",
        style(format!("Location: {}", config.completion_path.display())).cyan()
    );
    println!();
    println!("{}", style("Configuration instructions:").bold());
    println!("{}", config.config_instructions);
    println!();
    println!(
        "{}",
        style("After updating your shell config, restart your terminal or run:").dim()
    );
    match shell {
        Shell::Bash => println!("  source ~/.bashrc"),
        Shell::Zsh => println!("  exec zsh"),
        Shell::Fish => println!("  exec fish"),
        Shell::PowerShell => println!("  . $PROFILE"),
        _ => println!("  Restart your terminal"),
    }
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_completion_path_zsh() {
        let path = get_completion_path(Shell::Zsh).unwrap();
        assert!(path.to_string_lossy().contains(".zsh/completions/_aptu"));
    }

    #[test]
    fn test_get_completion_path_bash() {
        let path = get_completion_path(Shell::Bash).unwrap();
        assert!(path.to_string_lossy().contains(".bash_completion.d/aptu"));
    }

    #[test]
    fn test_get_completion_path_fish() {
        let path = get_completion_path(Shell::Fish).unwrap();
        assert!(
            path.to_string_lossy()
                .contains(".config/fish/completions/aptu.fish")
        );
    }

    #[test]
    fn test_shell_config_creation() {
        let config = ShellConfig::new(Shell::Zsh).unwrap();
        assert!(!config.config_instructions.is_empty());
    }

    #[test]
    fn test_config_instructions_not_empty() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let instructions = get_config_instructions(shell);
            assert!(
                !instructions.is_empty(),
                "Instructions empty for {:?}",
                shell
            );
        }
    }
}
