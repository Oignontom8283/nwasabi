//! `nwasabi-cli` — outil en ligne de commande pour tester l'émulateur (feature cargo `cli`).
//!
//! Contrairement à `nwasabi`, le cœur de l'émulateur est compilé dans ce binaire et
//! s'exécute nativement sur la machine hôte, sans navigateur. Les sorties `--json` sont
//! déterministes : une même exécution donne toujours les mêmes octets.

mod args;
mod commands;

const USAGE: &str = "\
nwasabi-cli — émulateur d'applications externes NumWorks (.nwa)

USAGE :
  nwasabi-cli <commande> [arguments]

COMMANDES :
  run <app.nwa>        exécute une application et résume l'arrêt, l'écran et la sortie standard
      --model n0110|n0115|n0120   --data <fichier>   --instructions N   --frames N
      --press <touche> (répétable)   --screenshot <fichier.png>   --json
  inspect [<app.nwa>]  structures de l'OS lues en mémoire (JSON) ; --model, --data
  link <app.nwa>       résultat de l'édition de liens ; --map ajoute sections et symboles ; --json
  fs --image <fichier> <op>   système de fichiers d'une image de 42 Kio :
      list | stats | read <nom> [--out f] | write <nom> (--from f | --text t) | remove <nom> | rename <a> <b>
  survey <dossier|fichiers…>  lance chaque application sans touche ; --instructions N ; --json
  session --stdio      une requête JSON par ligne sur l'entrée, une réponse par ligne en sortie
  keys                 noms des touches

Codes de sortie : 0 succès, 1 erreur, 2 mauvais usage, 3 l'application s'est arrêtée sur une faute.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let outcome = match command.as_str() {
        "run" => commands::run(rest),
        "inspect" => commands::inspect(rest),
        "link" => commands::link_cmd(rest),
        "fs" => commands::fs_cmd(rest),
        "survey" => commands::survey(rest),
        "session" => commands::session(rest),
        "keys" => commands::keys(rest),
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            return;
        }
        other => {
            eprintln!("commande inconnue : {other}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    match outcome {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("erreur : {message}");
            std::process::exit(1);
        }
    }
}
