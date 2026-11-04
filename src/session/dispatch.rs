//! L'aiguillage des commandes : le texte de `cmd` désigne la fonction qui la traite. Le code de chaque commande est dans le fichier de
//! son domaine ; ici, seul le `match`.

use super::{Reply, Session, aide, source, text};
use crate::json::Value;

impl Session {
    pub(super) fn dispatch(&mut self, req: &Value) -> Reply {
        let cmd = text(req, "cmd")?;
        match cmd {
            "help" => aide::help(),
            "new" => self.new_device(req),
            "load" => self.load(req),
            "launch" => self.launch(),
            "run" => self.run(req),
            "key" => self.key(req),
            "profile" => self.profile(req),
            "backtrace" => self.backtrace(req),
            "symbols" => self.symbols(req),
            "symbol" => self.symbol(req),
            "history" => self.history_command(req),
            "step_back" => self.step_back(),
            "reverse_continue" => self.reverse_continue(),
            "history_goto" => self.history_goto(req),
            "step" => self.step(),
            "break" => self.break_at(req),
            "unbreak" => self.unbreak(req),
            "breaks" => self.breaks(),
            "cpu_set" => self.cpu_set(req),
            "advance" => self.advance(req),
            "speed" => self.speed(req),
            "time" => self.time(),
            "screen" => self.screen(req),
            "stdout" => self.stdout(req),
            "cpu" => self.cpu(),
            "inspect" => self.inspect(),
            "regions" => self.regions(),
            "usage" => self.usage(),
            "memory_read" => self.memory_read(req),
            "memory_write" => self.memory_write(req),
            "changes" => self.changes(req),
            "watch" => self.watch(req),
            "unwatch" => self.unwatch(req),
            "disasm" => self.disasm(req),
            "watch_list" => self.watch_list(),
            "watch_hits" => self.watch_hits(),
            "svc_log" => self.svc_log(req),
            "svc_stats" => self.svc_stats(),
            "fs_export" => self.fs_export(),
            "fs_import" => self.fs_import(req),
            "flash_load" => self.flash_load(req),
            "flash_export" => self.flash_export(),
            "dump" => self.dump(req),
            "restore" => self.restore(req),
            c if source::COMMANDS.contains(&c) => self.source_command(c, req),
            c if c.starts_with("fs_") => self.fs(c, req),
            c if c.starts_with("settings_") => self.settings(c, req),
            other => Err(format!("commande inconnue : {other} (voir « help »)")),
        }
    }
}
