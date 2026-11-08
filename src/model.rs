//! Tipos compartidos entre el hilo de radio (tokio) y la interfaz (GTK).

use serde::{Deserialize, Serialize};

pub const BROADCAST: u32 = u32::MAX;

/// Cómo llegar al nodo.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Target {
    Ble { name: String, mac: String },
    Serial { port: String },
    Tcp { addr: String },
}

impl Target {
    pub fn label(&self) -> String {
        match self {
            Target::Ble { name, mac } => format!("Bluetooth: {name} ({mac})"),
            Target::Serial { port } => format!("USB: {port}"),
            Target::Tcp { addr } => format!("Red: {addr}"),
        }
    }
}

/// Una conversación: un canal o un chat directo con un nodo.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Conv {
    Channel(u32),
    Direct(u32),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Delivery {
    /// Mensaje recibido.
    Incoming,
    /// Enviado, esperando confirmación.
    Pending,
    /// Confirmado por la malla o por el destinatario.
    Acked,
    /// La radio informó de un error.
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Msg {
    pub id: u32,
    pub conv: Conv,
    pub from: u32,
    pub text: String,
    pub time: u32,
    pub delivery: Delivery,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub snr: Option<f32>,
    #[serde(default)]
    pub rssi: Option<i32>,
    #[serde(default)]
    pub hops: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Node {
    pub num: u32,
    pub long_name: String,
    pub short_name: String,
    pub hw_model: String,
    pub role: String,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
    pub last_heard: u32,
    pub hops_away: Option<u32>,
    pub via_mqtt: bool,
    pub battery: Option<u32>,
    pub voltage: Option<f32>,
    pub channel_util: Option<f32>,
    pub air_util_tx: Option<f32>,
    pub uptime: Option<u32>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt: Option<i32>,
    pub has_pubkey: bool,
    pub favorite: bool,
}

impl Node {
    pub fn id_str(num: u32) -> String {
        format!("!{num:08x}")
    }

    pub fn display_name(&self) -> String {
        if !self.long_name.is_empty() {
            self.long_name.clone()
        } else if !self.short_name.is_empty() {
            self.short_name.clone()
        } else {
            Self::id_str(self.num)
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChannelInfo {
    pub index: u32,
    pub name: String,
    /// 1 = primario, 2 = secundario. Los deshabilitados no se envían a la UI.
    pub role: i32,
}

/// Órdenes de la UI al hilo de radio.
#[derive(Debug)]
pub enum Command {
    ScanBle,
    Connect(Target),
    Disconnect,
    SendText { conv: Conv, text: String },
}

/// Avisos del hilo de radio a la UI.
#[derive(Debug)]
pub enum Event {
    BleDevices(Result<Vec<(String, String)>, String>),
    Connecting(Target),
    Connected(Target),
    Disconnected(Option<String>),
    MyNode(u32),
    ConfigComplete,
    Node(Node),
    /// Actualización parcial de un nodo a partir de un paquete recibido.
    NodePatch(u32, NodePatch),
    Channel(ChannelInfo),
    Message(Msg),
    Delivery { id: u32, ok: bool, error: Option<String> },
    Error(String),
}

#[derive(Debug, Default)]
pub struct NodePatch {
    pub heard_at: Option<u32>,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
    pub hops: Option<u32>,
    pub user: Option<(String, String, String, String, bool)>,
    pub position: Option<(f64, f64, Option<i32>)>,
    pub battery: Option<u32>,
    pub voltage: Option<f32>,
    pub channel_util: Option<f32>,
    pub air_util_tx: Option<f32>,
    pub uptime: Option<u32>,
}

pub fn now() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}
