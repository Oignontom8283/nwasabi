//! Les arguments de la ligne de commande, décrits pour `clap`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Entier décimal (avec `_` possibles) ou hexadécimal (`0x…`).
fn count(text: &str) -> Result<u64, String> {
    let parsed = match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.replace('_', "").parse(),
    };
    parsed.map_err(|_| format!("« {text} » n'est pas un entier"))
}

/// Modèle de calculatrice.
#[derive(Clone, Copy, ValueEnum)]
pub enum ModelOpt {
    N0110,
    N0115,
    N0120,
}

impl ModelOpt {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::N0110 => "n0110",
            Self::N0115 => "n0115",
            Self::N0120 => "n0120",
        }
    }
}

/// Disposition du firmware émulé.
#[derive(Clone, Copy, ValueEnum)]
pub enum FirmwareOpt {
    /// Appareils relevés (Epsilon 23–24) : celle que supposent les applications eadkp.
    #[value(name = "24")]
    V24,
    /// Epsilon 26.4.0 officiel.
    #[value(name = "26.4.0")]
    V26,
}

impl FirmwareOpt {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V24 => "24",
            Self::V26 => "26.4.0",
        }
    }
}

/// Appareil à émuler.
#[derive(Args)]
pub struct DeviceOpts {
    /// Modèle de calculatrice
    #[arg(long, value_enum, ignore_case = true, default_value = "n0110")]
    pub model: ModelOpt,
    /// Disposition du firmware
    #[arg(long, value_enum, default_value = "24")]
    pub firmware: FirmwareOpt,
    /// Données externes de l'application (ROM, script…)
    #[arg(long, value_name = "FICHIER")]
    pub data: Option<PathBuf>,
    /// Octets à placer en flash à côté de l'application (fichier .nfd, voir « flash pack »)
    #[arg(long, value_name = "FICHIER.nfd")]
    pub flash: Option<PathBuf>,
    /// Fréquence du processeur en MHz, qui règle le temps virtuel (par défaut : celle du modèle)
    #[arg(long, value_name = "MHZ")]
    pub mhz: Option<u32>,
}

/// nwasabi-cli : émulateur d'applications externes NumWorks (.nwa).
///
/// Codes de sortie : 0 succès, 1 erreur, 2 mauvais usage, 3 l'application s'est arrêtée sur une faute.
#[derive(Parser)]
#[command(name = "nwasabi-cli", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Exécute une application et résume l'arrêt, l'écran et la sortie standard
    Run(RunArgs),
    /// Affiche les structures de l'OS lues en mémoire (JSON)
    Inspect(InspectArgs),
    /// Affiche le résultat de l'édition de liens
    Link(LinkArgs),
    /// Manipule une image du système de fichiers (tampon de 42 Kio)
    Fs(FsArgs),
    /// Lance chaque application d'un dossier sans touche et résume ce qui s'est passé
    Survey(SurveyArgs),
    /// Une requête JSON par ligne sur l'entrée standard, une réponse par ligne en sortie
    Session(SessionArgs),
    /// Liste les noms des touches
    Keys(KeysArgs),
    /// Fabrique ou lit un fichier .nfd (octets à placer en flash)
    Flash(FlashArgs),
}

#[derive(Args)]
pub struct FlashArgs {
    /// Sortie en JSON
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub op: FlashOp,
}

#[derive(Subcommand)]
pub enum FlashOp {
    /// Rassemble des fichiers à leur adresse dans un .nfd
    Pack {
        /// Fichier .nfd à écrire
        #[arg(long, short, value_name = "FICHIER.nfd")]
        out: PathBuf,
        /// Slot du firmware que l'appareil doit exécuter (quand les segments occupent l'autre)
        #[arg(long, value_name = "a|b", value_parser = ["a", "b"])]
        active_slot: Option<String>,
        /// Les segments, de la forme ADRESSE=FICHIER (par exemple 0x90400000=khi120ab.tar)
        #[arg(required = true, value_name = "ADRESSE=FICHIER")]
        segments: Vec<String>,
    },
    /// Liste les segments d'un .nfd
    List {
        /// Fichier .nfd
        file: PathBuf,
    },
}

#[derive(Args)]
pub struct RunArgs {
    /// Fichier .nwa (facultatif avec --nds, qui peut contenir l'application)
    #[arg(required_unless_present = "nds")]
    pub app: Option<PathBuf>,
    /// Reprend l'état enregistré dans ce fichier .nds (modèle et firmware viennent du fichier)
    #[arg(long, value_name = "FICHIER.nds")]
    pub nds: Option<PathBuf>,
    /// Charge les fichiers de ce fichier .nfs dans la calculatrice avant de lancer
    #[arg(long, value_name = "FICHIER.nfs")]
    pub nfs: Option<PathBuf>,
    /// Enregistre l'état de la calculatrice dans ce fichier .nds après l'exécution
    #[arg(long, value_name = "FICHIER.nds")]
    pub save_nds: Option<PathBuf>,
    /// Enregistre les fichiers de la calculatrice dans ce fichier .nfs après l'exécution
    #[arg(long, value_name = "FICHIER.nfs")]
    pub save_nfs: Option<PathBuf>,
    #[command(flatten)]
    pub device: DeviceOpts,
    /// Nombre maximal d'instructions (défaut : 10 millions)
    #[arg(long, value_parser = count)]
    pub instructions: Option<u64>,
    /// S'arrêter après ce nombre d'images
    #[arg(long, value_parser = count)]
    pub frames: Option<u64>,
    /// Touche enfoncée avant de lancer (répétable ; voir « keys »). Avec --nmk, une touche de l'ordinateur
    /// (KeyW, key:e…) est aussi acceptée
    #[arg(long, value_name = "TOUCHE")]
    pub press: Vec<String>,
    /// Mapping des touches (fichier .nmk) pour traduire les touches de --press
    #[arg(long, value_name = "FICHIER.nmk")]
    pub nmk: Option<PathBuf>,
    /// Enregistre l'écran final en PNG
    #[arg(long, value_name = "FICHIER.png")]
    pub screenshot: Option<PathBuf>,
    /// Sortie en JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct InspectArgs {
    /// Fichier .nwa à installer d'abord (sinon : appareil vide)
    pub app: Option<PathBuf>,
    #[command(flatten)]
    pub device: DeviceOpts,
}

#[derive(Args)]
pub struct LinkArgs {
    /// Fichier .nwa
    pub app: PathBuf,
    #[command(flatten)]
    pub device: DeviceOpts,
    /// Ajoute les sections et les symboles
    #[arg(long)]
    pub map: bool,
    /// Écrit l'application liée au format ELF (symboles, sections, DWARF) : à donner à GDB avec `file`
    #[arg(long, value_name = "FICHIER.elf")]
    pub elf: Option<PathBuf>,
    /// Sortie en JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct FsArgs {
    /// Fichier image (0xA800 octets ; créé à la première écriture)
    #[arg(long, value_name = "FICHIER")]
    pub image: PathBuf,
    /// Sortie en JSON
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub op: FsOp,
}

#[derive(Subcommand)]
pub enum FsOp {
    /// Liste les fichiers
    List,
    /// Occupation du tampon
    Stats,
    /// Écrit le contenu d'un fichier (sur la sortie standard, ou dans --out)
    Read {
        name: String,
        #[arg(long, value_name = "FICHIER")]
        out: Option<PathBuf>,
    },
    /// Crée ou remplace un fichier
    Write {
        name: String,
        /// Contenu pris dans ce fichier
        #[arg(
            long,
            value_name = "FICHIER",
            conflicts_with = "text",
            required_unless_present = "text"
        )]
        from: Option<PathBuf>,
        /// Contenu donné directement
        #[arg(long)]
        text: Option<String>,
    },
    /// Supprime un fichier
    Remove { name: String },
    /// Renomme un fichier
    Rename { from: String, to: String },
    /// Enregistre tous les fichiers dans un fichier .nfs
    Export { to: PathBuf },
    /// Charge les fichiers d'un fichier .nfs (remplace les fichiers existants, sauf avec --keep)
    Import {
        from: PathBuf,
        /// Garde les fichiers existants (ceux du même nom sont remplacés)
        #[arg(long)]
        keep: bool,
    },
}

#[derive(Args)]
pub struct SurveyArgs {
    /// Dossiers (de fichiers .nwa) ou fichiers .nwa
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
    /// Instructions par application (défaut : 40 millions)
    #[arg(long, value_parser = count)]
    pub instructions: Option<u64>,
    /// Sortie en JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct SessionArgs {
    /// Requêtes sur l'entrée standard (option conservée pour la clarté)
    #[arg(long)]
    pub stdio: bool,
}

#[derive(Args)]
pub struct KeysArgs {
    /// Affiche plutôt le mapping d'un fichier .nmk (complété par le défaut) : touches de l'ordinateur, gestes
    #[arg(long, value_name = "FICHIER.nmk")]
    pub nmk: Option<PathBuf>,
    /// Sortie en JSON
    #[arg(long)]
    pub json: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_line_description_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn counts_accept_hex_and_separators() {
        assert_eq!(count("0x10"), Ok(16));
        assert_eq!(count("40_000_000"), Ok(40_000_000));
        assert!(count("x").is_err());
    }

    #[test]
    fn a_run_command_is_parsed() {
        let cli = Cli::try_parse_from([
            "nwasabi-cli",
            "run",
            "app.nwa",
            "--model",
            "n0120",
            "--firmware",
            "26.4.0",
            "--instructions",
            "1_000",
            "--press",
            "ok",
            "--press",
            "up",
            "--json",
        ])
        .unwrap();
        let Command::Run(run) = cli.command else {
            panic!("run attendu");
        };
        assert_eq!(run.device.model.as_str(), "n0120");
        assert_eq!(run.device.firmware.as_str(), "26.4.0");
        assert_eq!(run.instructions, Some(1000));
        assert_eq!(run.press, ["ok", "up"]);
        assert!(run.json);
    }

    #[test]
    fn mistakes_are_refused() {
        assert!(Cli::try_parse_from(["nwasabi-cli", "run"]).is_err());
        assert!(Cli::try_parse_from(["nwasabi-cli", "run", "a.nwa", "--model", "n9999"]).is_err());
        assert!(Cli::try_parse_from(["nwasabi-cli", "fs", "list"]).is_err());
        assert!(
            Cli::try_parse_from(["nwasabi-cli", "fs", "--image", "i", "write", "a.py"]).is_err()
        );
    }
}
