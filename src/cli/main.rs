//! `nwasabi-cli` — outil en ligne de commande pour tester l'émulateur (feature cargo `cli`).
//!
//! Contrairement à `nwasabi`, le cœur de l'émulateur est compilé dans ce binaire et
//! s'exécute nativement sur la machine hôte, sans navigateur. Les sorties `--json` sont
//! déterministes : une même exécution donne toujours les mêmes octets.

mod commands;
mod options;

use clap::Parser;

use options::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let outcome = match cli.command {
        Command::Run(args) => commands::run(args),
        Command::Inspect(args) => commands::inspect(args),
        Command::Link(args) => commands::link_cmd(args),
        Command::Fs(args) => commands::fs_cmd(args),
        Command::Survey(args) => commands::survey(args),
        Command::Session(args) => commands::session(args),
        Command::Keys(args) => commands::keys(args),
        Command::Flash(args) => commands::flash_cmd(args),
        Command::Replay(args) => commands::replay_cmd(args),
    };
    match outcome {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("erreur : {message}");
            std::process::exit(1);
        }
    }
}
