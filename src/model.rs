//! Tipos compartidos entre el hilo de radio (tokio) y la interfaz (GTK).

use meshtastic::protobufs;
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
            Target::Ble { name, mac } if name.is_empty() => format!("Bluetooth: {mac}"),
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
    /// Id del mensaje al que responde o reacciona.
    #[serde(default)]
    pub reply_id: Option<u32>,
    /// Es una reacción con emoji (el texto es el emoji).
    #[serde(default)]
    pub emoji: bool,
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
    pub ignored: bool,
    pub licensed: bool,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub pressure: Option<f32>,
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

    pub fn short(&self) -> String {
        if !self.short_name.is_empty() {
            self.short_name.clone()
        } else {
            format!("{:04x}", self.num & 0xffff)
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

/// Una medida de telemetría de un nodo, para las gráficas.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sample {
    pub t: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voltage: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch_util: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub air_tx: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snr: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temp: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hum: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub press: Option<f32>,
}

/// Resultado de un traceroute: la ruta de ida y la de vuelta con su SNR por salto.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trace {
    pub target: u32,
    pub time: u32,
    /// Nodos intermedios de ida y SNR en cada recepción (uno más que nodos).
    pub towards: Vec<u32>,
    pub snr_towards: Vec<Option<f32>>,
    pub back: Vec<u32>,
    pub snr_back: Vec<Option<f32>>,
    pub has_back: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Waypoint {
    pub id: u32,
    pub lat: f64,
    pub lon: f64,
    pub name: String,
    pub description: String,
    /// Emoji como código Unicode (0 = sin icono).
    pub icon: u32,
    /// Caduca a esta hora Unix (0 = nunca).
    pub expire: u32,
    pub locked_to: u32,
    pub from: u32,
    pub time: u32,
}

impl Waypoint {
    pub fn icon_str(&self) -> String {
        char::from_u32(self.icon).filter(|_| self.icon != 0).map(String::from).unwrap_or_else(|| "📍".into())
    }
}

/// Una línea del registro de paquetes.
#[derive(Clone, Debug)]
pub struct PacketLog {
    pub time: u32,
    pub from: u32,
    pub to: u32,
    pub port: String,
    pub channel: u32,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
    pub hops: Option<u32>,
    pub size: usize,
    pub encrypted: bool,
    pub via_mqtt: bool,
    pub summary: String,
}

/// Órdenes de la UI al hilo de radio.
#[derive(Debug)]
pub enum Command {
    ScanBle,
    Connect(Target),
    Disconnect,
    SendText { conv: Conv, text: String, reply_id: Option<u32>, emoji: bool },
    Traceroute(u32),
    RequestPosition(u32),
    RequestUser(u32),
    SendWaypoint(protobufs::Waypoint),
    /// Mensajes de administración al nodo conectado. Si alguno cambia la
    /// configuración, el hilo de radio los envuelve en una transacción.
    Admin(Vec<protobufs::admin_message::PayloadVariant>),
}

/// Avisos del hilo de radio a la UI.
#[derive(Debug)]
pub enum Event {
    BleDevices(Result<Vec<(String, String)>, String>),
    Connecting(Target),
    Connected(Target),
    Disconnected(Option<String>),
    /// La conexión se cayó y se reintenta sola.
    Reconnecting { target: Target, secs: u64, reason: String },
    MyNode(u32),
    ConfigComplete,
    Node(Node),
    /// Actualización parcial de un nodo a partir de un paquete recibido.
    NodePatch(u32, NodePatch),
    Channel(ChannelInfo),
    ChannelFull(protobufs::Channel),
    Config(protobufs::Config),
    ModuleConfig(protobufs::ModuleConfig),
    Metadata(protobufs::DeviceMetadata),
    MyUser(protobufs::User),
    Message(Msg),
    Delivery { id: u32, ok: bool, error: Option<String> },
    Trace(Trace),
    Waypoint(Waypoint),
    Packet(PacketLog),
    Info(String),
    Error(String),
}

#[derive(Debug, Default)]
pub struct NodePatch {
    pub heard_at: Option<u32>,
    pub snr: Option<f32>,
    pub rssi: Option<i32>,
    pub hops: Option<u32>,
    pub via_mqtt: Option<bool>,
    pub user: Option<protobufs::User>,
    pub position: Option<(f64, f64, Option<i32>)>,
    pub battery: Option<u32>,
    pub voltage: Option<f32>,
    pub channel_util: Option<f32>,
    pub air_util_tx: Option<f32>,
    pub uptime: Option<u32>,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub pressure: Option<f32>,
}

impl NodePatch {
    pub fn has_telemetry(&self) -> bool {
        self.battery.is_some()
            || self.voltage.is_some()
            || self.channel_util.is_some()
            || self.temperature.is_some()
            || self.humidity.is_some()
            || self.pressure.is_some()
    }
}

pub fn now() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

pub fn hw_name(v: i32) -> String {
    protobufs::HardwareModel::try_from(v)
        .map(|h| h.as_str_name().to_string())
        .unwrap_or_else(|_| format!("modelo {v}"))
}

pub fn role_name(v: i32) -> String {
    protobufs::config::device_config::Role::try_from(v)
        .map(|r| r.as_str_name().to_string())
        .unwrap_or_default()
}

/// Distancia en metros (haversine) y rumbo en grados entre dos puntos.
pub fn distance_bearing(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let r = 6_371_000.0;
    let (la1, lo1, la2, lo2) = (a.0.to_radians(), a.1.to_radians(), b.0.to_radians(), b.1.to_radians());
    let dla = la2 - la1;
    let dlo = lo2 - lo1;
    let h = (dla / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.0).sin().powi(2);
    let d = 2.0 * r * h.sqrt().asin();
    let y = dlo.sin() * la2.cos();
    let x = la1.cos() * la2.sin() - la1.sin() * la2.cos() * dlo.cos();
    let brg = (y.atan2(x).to_degrees() + 360.0) % 360.0;
    (d, brg)
}

pub fn fmt_distance(m: f64) -> String {
    if m < 1000.0 {
        format!("{m:.0} m")
    } else if m < 10_000.0 {
        format!("{:.1} km", m / 1000.0)
    } else {
        format!("{:.0} km", m / 1000.0)
    }
}

pub fn compass(deg: f64) -> &'static str {
    const P: [&str; 8] = ["N", "NE", "E", "SE", "S", "SO", "O", "NO"];
    P[(((deg + 22.5) % 360.0) / 45.0) as usize % 8]
}
