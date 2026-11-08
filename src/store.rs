//! Guardado en disco: historial de mensajes y última conexión.
//! Ruta: ~/.local/share/malla/ (o $XDG_DATA_HOME/malla).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::{Msg, Target};

const MAX_MSGS: usize = 5000;

static DEMO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// En modo demo no se lee ni se escribe nada en disco.
pub fn set_demo() {
    DEMO.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_demo() -> bool {
    DEMO.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Default, Serialize, Deserialize)]
pub struct Saved {
    #[serde(default)]
    pub last_target: Option<Target>,
    #[serde(default)]
    pub messages: Vec<Msg>,
    /// Nombres de nodos conocidos, para mostrar el historial sin conexión.
    #[serde(default)]
    pub names: std::collections::BTreeMap<u32, String>,
}

fn dir() -> PathBuf {
    gtk::glib::user_data_dir().join("malla")
}

fn file() -> PathBuf {
    dir().join("estado.json")
}

pub fn load() -> Saved {
    if is_demo() {
        return Saved::default();
    }
    std::fs::read(file())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save(s: &mut Saved) {
    if is_demo() {
        return;
    }
    if s.messages.len() > MAX_MSGS {
        let extra = s.messages.len() - MAX_MSGS;
        s.messages.drain(..extra);
    }
    let _ = std::fs::create_dir_all(dir());
    let tmp = dir().join("estado.json.tmp");
    if let Ok(b) = serde_json::to_vec(s) {
        if std::fs::write(&tmp, b).is_ok() {
            let _ = std::fs::rename(tmp, file());
        }
    }
}
