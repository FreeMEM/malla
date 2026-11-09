//! Guardado en disco: historial, datos recogidos de la malla y preferencias.
//! Ruta: ~/.local/share/malla/ (o $XDG_DATA_HOME/malla).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::{Conv, Msg, Sample, Target, Trace, Waypoint};

const MAX_MSGS: usize = 5000;
const MAX_SAMPLES: usize = 500;
const MAX_TRACK: usize = 300;
const MAX_TRACES: usize = 100;

static DEMO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// En modo demo no se lee ni se escribe nada en disco.
pub fn set_demo() {
    DEMO.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_demo() -> bool {
    DEMO.load(std::sync::atomic::Ordering::Relaxed)
}

fn yes() -> bool {
    true
}

/// Una respuesta automática: si un mensaje contiene `keyword`, se contesta `reply`.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoRule {
    pub keyword: String,
    pub reply: String,
    /// 0 = solo mensajes directos, 1 = solo canales, 2 = los dos.
    #[serde(default)]
    pub scope: u8,
}

fn default_rules() -> Vec<AutoRule> {
    vec![AutoRule { keyword: "ping".into(), reply: "pong 🏓 Te recibo con SNR {snr} dB, {saltos}.".into(), scope: 0 }]
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Prefs {
    /// Avisos de escritorio al llegar mensajes.
    #[serde(default = "yes")]
    pub notify: bool,
    /// Avisar también de los mensajes de canal (no solo directos).
    #[serde(default = "yes")]
    pub notify_channels: bool,
    /// Arrancar escondida en la bandeja.
    #[serde(default)]
    pub start_hidden: bool,
    /// "sistema", "claro" u "oscuro".
    #[serde(default)]
    pub theme: String,
    /// Fuente de teselas del mapa.
    #[serde(default)]
    pub map_source: String,
    /// Mapa oscuro.
    #[serde(default)]
    pub map_night: bool,
    /// Tamaño de la ventana (ancho, alto) y si estaba maximizada.
    #[serde(default)]
    pub window: Option<(i32, i32, bool)>,
    /// Mirar en GitHub si hay una versión nueva al arrancar.
    #[serde(default)]
    pub check_updates: bool,
    /// Respuestas rápidas.
    #[serde(default = "default_quick")]
    pub quick: Vec<String>,
    /// Respuestas automáticas activadas.
    #[serde(default)]
    pub auto_on: bool,
    #[serde(default = "default_rules")]
    pub auto_rules: Vec<AutoRule>,
}

fn default_quick() -> Vec<String> {
    ["¿Me recibes?", "Recibido 👍", "Voy de camino", "Todo bien por aquí", "¿Dónde estás?", "Llámame cuando puedas"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            notify: true,
            notify_channels: true,
            start_hidden: false,
            theme: String::new(),
            map_source: String::new(),
            map_night: false,
            window: None,
            check_updates: false,
            quick: default_quick(),
            auto_on: false,
            auto_rules: default_rules(),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Saved {
    #[serde(default)]
    pub last_target: Option<Target>,
    /// Últimas conexiones distintas, la más reciente primero.
    #[serde(default)]
    pub recent: Vec<Target>,
    #[serde(default)]
    pub messages: Vec<Msg>,
    /// Nombres de nodos conocidos, para mostrar el historial sin conexión.
    #[serde(default)]
    pub names: BTreeMap<u32, String>,
    #[serde(default)]
    pub telemetry: BTreeMap<u32, Vec<Sample>>,
    /// Posiciones recibidas de cada nodo: (hora, lat, lon).
    #[serde(default)]
    pub tracks: BTreeMap<u32, Vec<(u32, f64, f64)>>,
    #[serde(default)]
    pub traces: Vec<Trace>,
    #[serde(default)]
    pub waypoints: BTreeMap<u32, Waypoint>,
    #[serde(default)]
    pub muted: Vec<Conv>,
    #[serde(default)]
    pub prefs: Prefs,
}

impl Saved {
    pub fn add_sample(&mut self, num: u32, s: Sample) {
        let v = self.telemetry.entry(num).or_default();
        v.push(s);
        if v.len() > MAX_SAMPLES {
            let extra = v.len() - MAX_SAMPLES;
            v.drain(..extra);
        }
    }

    pub fn add_track(&mut self, num: u32, t: u32, la: f64, lo: f64) {
        let v = self.tracks.entry(num).or_default();
        if v.last().map(|p| (p.1 - la).abs() < 1e-6 && (p.2 - lo).abs() < 1e-6).unwrap_or(false) {
            return; // misma posición
        }
        v.push((t, la, lo));
        if v.len() > MAX_TRACK {
            let extra = v.len() - MAX_TRACK;
            v.drain(..extra);
        }
    }

    pub fn add_trace(&mut self, t: Trace) {
        self.traces.push(t);
        if self.traces.len() > MAX_TRACES {
            let extra = self.traces.len() - MAX_TRACES;
            self.traces.drain(..extra);
        }
    }

    pub fn add_recent(&mut self, t: Target) {
        self.recent.retain(|x| *x != t);
        self.recent.insert(0, t);
        self.recent.truncate(5);
    }

    pub fn is_muted(&self, c: Conv) -> bool {
        self.muted.contains(&c)
    }
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

/// Lanzador en ~/.config/autostart para arrancar con la sesión.
pub fn autostart_path() -> PathBuf {
    gtk::glib::user_config_dir().join("autostart").join("es.freemem.Malla.desktop")
}

pub fn set_autostart(on: bool) -> std::io::Result<()> {
    let p = autostart_path();
    if on {
        std::fs::create_dir_all(p.parent().unwrap())?;
        let exe = std::env::current_exe().map(|e| e.display().to_string()).unwrap_or_else(|_| "malla".into());
        std::fs::write(
            &p,
            format!(
                "[Desktop Entry]\nType=Application\nName=Malla\nExec={exe} --oculta\nIcon=es.freemem.Malla\nX-GNOME-Autostart-enabled=true\n"
            ),
        )
    } else if p.exists() {
        std::fs::remove_file(p)
    } else {
        Ok(())
    }
}
