//! Shell completion generation (feature **`completions`**).
//!
//! # Usage
//!
//! ```sh
//! # bash  (~/.bash_completion.d/imgvwr)
//! imgvwr --completions bash > ~/.bash_completion.d/imgvwr
//!
//! # zsh  (somewhere in $fpath, e.g. /usr/share/zsh/site-functions/_imgvwr)
//! imgvwr --completions zsh > ~/.local/share/zsh/site-functions/_imgvwr
//!
//! # fish  (~/.config/fish/completions/imgvwr.fish)
//! imgvwr --completions fish > ~/.config/fish/completions/imgvwr.fish
//!
//! # nushell  (add to $nu.completion-path)
//! imgvwr --completions nushell > ~/.config/nushell/completions/imgvwr.nu
//! ```

use clap::CommandFactory as _;
use clap_complete::generate;

use crate::cli::Cli;

/// Supported shells for completion generation.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Nushell,
}

/// Generate a completion script for `shell` and write it to stdout.
pub fn generate_completions(shell: CompletionShell) {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    let stdout = &mut std::io::stdout();
    match shell {
        CompletionShell::Bash => generate(clap_complete::Shell::Bash, &mut cmd, name, stdout),
        CompletionShell::Zsh => generate(clap_complete::Shell::Zsh, &mut cmd, name, stdout),
        CompletionShell::Fish => generate(clap_complete::Shell::Fish, &mut cmd, name, stdout),
        CompletionShell::Nushell => {
            generate(clap_complete_nushell::Nushell, &mut cmd, name, stdout)
        }
    }
}

#[cfg(test)]
mod tests;
