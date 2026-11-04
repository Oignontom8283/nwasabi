//! Les fichiers de la calculatrice : lecture et écriture en bloc (`.nfs`).

use super::{Reply, Session, body_of, text, views};
use crate::formats::nfs;
use crate::json;
use crate::json::Value;
use crate::json::ValueExt;

use crate::storage::PythonSource;

impl Session {
    pub(super) fn fs(&mut self, cmd: &str, req: &Value) -> Reply {
        let emu = self.emu()?;
        let mut fs = emu.fs();
        let e = |e: crate::storage::FsError| e.to_string();
        match cmd {
            "fs_list" => Ok(views::listing(&fs.list().map_err(e)?)
                .with("generation", fs.generation().map_err(e)?)),
            "fs_stats" => Ok(views::stats(&fs.stats().map_err(e)?)),
            "fs_read" => {
                let name = text(req, "name")?;
                if req.get("python").and_then(Value::as_bool) == Some(true) {
                    let src = fs.read_python(name).map_err(e)?;
                    return Ok(Value::object()
                        .with("auto_import", src.auto_import)
                        .with("text", src.text));
                }
                let body = fs.read(name).map_err(e)?;
                Ok(Value::object()
                    .with("size", body.len())
                    .with("content", json::base64_encode(&body)))
            }
            "fs_write" => {
                let name = text(req, "name")?;
                if req.get("python").and_then(Value::as_bool) == Some(true) {
                    let source = PythonSource {
                        auto_import: req
                            .get("auto_import")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        text: text(req, "text")?.to_owned(),
                    };
                    fs.write_python(name, &source).map_err(e)?;
                } else {
                    fs.write(name, &body_of(req)?).map_err(e)?;
                }
                Ok(Value::object())
            }
            "fs_remove" => {
                fs.remove(text(req, "name")?).map_err(e)?;
                Ok(Value::object())
            }
            "fs_rename" => {
                fs.rename(text(req, "from")?, text(req, "to")?).map_err(e)?;
                Ok(Value::object())
            }
            other => Err(format!("commande inconnue : {other}")),
        }
    }

    /// La commande `fs_export`.
    pub(super) fn fs_export(&mut self) -> Reply {
        let doc = nfs::export(&mut self.emu()?.fs())?;
        Ok(Value::object().with("nfs", doc))
    }

    /// La commande `fs_import`.
    pub(super) fn fs_import(&mut self, req: &Value) -> Reply {
        let doc = req.get("nfs").ok_or("champ « nfs » (document) manquant")?;
        let replace = req.get("replace").and_then(Value::as_bool).unwrap_or(true);
        let count = nfs::import(&mut self.emu()?.fs(), doc, replace)?;
        Ok(Value::object().with("files", count))
    }
}
