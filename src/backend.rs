//! Hilo de radio: habla con el nodo Meshtastic usando tokio y avisa a la UI.

use std::convert::Infallible;
use std::time::Duration;

use meshtastic::api::{ConnectedStreamApi, StreamApi};
use meshtastic::packet::{PacketDestination, PacketReceiver, PacketRouter};
use meshtastic::protobufs::{
    self, from_radio, mesh_packet, routing, telemetry, FromRadio, MeshPacket, PortNum,
};
use meshtastic::types::{MeshChannel, NodeId};
use meshtastic::utils;
use meshtastic::utils::stream::BleId;
use meshtastic::Message as _;
use tokio::sync::mpsc;

use crate::model::*;

type Ui = async_channel::Sender<Event>;

fn emit(ui: &Ui, e: Event) {
    let _ = ui.try_send(e);
}

/// Arranca el hilo de radio. Devuelve el canal para mandarle órdenes.
pub fn spawn(ui: Ui) -> mpsc::UnboundedSender<Command> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::Builder::new()
        .name("radio".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("no se pudo crear el runtime de tokio");
            rt.block_on(run(rx, ui));
        })
        .expect("no se pudo crear el hilo de radio");
    tx
}

/// La librería pide un "router" para saber nuestro id y devolvernos el eco de lo enviado.
struct Router {
    my: u32,
    ui: Ui,
}

impl PacketRouter<(), Infallible> for Router {
    fn handle_packet_from_radio(&mut self, _packet: FromRadio) -> Result<(), Infallible> {
        Ok(())
    }

    /// Eco de un paquete que acabamos de enviar: así conocemos su id para la confirmación.
    fn handle_mesh_packet(&mut self, p: MeshPacket) -> Result<(), Infallible> {
        if let Some(mesh_packet::PayloadVariant::Decoded(d)) = &p.payload_variant {
            if d.portnum == PortNum::TextMessageApp as i32 {
                let conv = if p.to == BROADCAST {
                    Conv::Channel(p.channel)
                } else {
                    Conv::Direct(p.to)
                };
                emit(
                    &self.ui,
                    Event::Message(Msg {
                        id: p.id,
                        conv,
                        from: self.my,
                        text: String::from_utf8_lossy(&d.payload).into_owned(),
                        time: now(),
                        delivery: Delivery::Pending,
                        error: None,
                        snr: None,
                        rssi: None,
                        hops: None,
                    }),
                );
            }
        }
        Ok(())
    }

    fn source_node_id(&self) -> NodeId {
        NodeId::new(self.my)
    }
}

struct Conn {
    api: ConnectedStreamApi,
    rx: PacketReceiver,
}

enum Step {
    Cmd(Option<Command>),
    Pkt(Option<FromRadio>),
}

async fn run(mut cmds: mpsc::UnboundedReceiver<Command>, ui: Ui) {
    let mut conn: Option<Conn> = None;
    let mut router = Router { my: 0, ui: ui.clone() };

    loop {
        let step = match conn.as_mut() {
            Some(c) => tokio::select! {
                cmd = cmds.recv() => Step::Cmd(cmd),
                pkt = c.rx.recv() => Step::Pkt(pkt),
            },
            None => Step::Cmd(cmds.recv().await),
        };

        match step {
            Step::Cmd(None) => break,
            Step::Cmd(Some(cmd)) => handle_cmd(cmd, &mut conn, &mut router, &ui).await,
            Step::Pkt(Some(p)) => handle_from_radio(p, &mut router, &ui),
            Step::Pkt(None) => {
                conn = None;
                emit(&ui, Event::Disconnected(Some("Se perdió la conexión con el nodo".into())));
            }
        }
    }

    if let Some(c) = conn {
        let _ = c.api.disconnect().await;
    }
}

async fn handle_cmd(cmd: Command, conn: &mut Option<Conn>, router: &mut Router, ui: &Ui) {
    match cmd {
        Command::ScanBle => {
            let ui = ui.clone();
            tokio::spawn(async move {
                let res = utils::stream::available_ble_devices(Duration::from_secs(6))
                    .await
                    .map(|v| {
                        v.into_iter()
                            .map(|d| {
                                (
                                    d.name.unwrap_or_else(|| "(sin nombre)".into()),
                                    d.mac_address.to_string(),
                                )
                            })
                            .collect()
                    })
                    .map_err(|e| e.to_string());
                emit(&ui, Event::BleDevices(res));
            });
        }
        Command::Connect(target) => {
            if let Some(c) = conn.take() {
                let _ = c.api.disconnect().await;
            }
            router.my = 0;
            emit(ui, Event::Connecting(target.clone()));
            match tokio::time::timeout(Duration::from_secs(40), open(&target)).await {
                Ok(Ok(c)) => {
                    *conn = Some(c);
                    emit(ui, Event::Connected(target));
                }
                Ok(Err(e)) => emit(ui, Event::Disconnected(Some(e))),
                Err(_) => emit(
                    ui,
                    Event::Disconnected(Some("El nodo no respondió a tiempo".into())),
                ),
            }
        }
        Command::Disconnect => {
            if let Some(c) = conn.take() {
                let _ = c.api.disconnect().await;
            }
            emit(ui, Event::Disconnected(None));
        }
        Command::SendText { conv, text } => {
            let Some(c) = conn.as_mut() else {
                emit(ui, Event::Error("No hay conexión con el nodo".into()));
                return;
            };
            let (dest, ch) = match conv {
                Conv::Channel(ch) => (PacketDestination::Broadcast, ch),
                Conv::Direct(n) => (PacketDestination::Node(NodeId::new(n)), 0),
            };
            if let Err(e) = c
                .api
                .send_text(router, text, dest, true, MeshChannel::from(ch))
                .await
            {
                emit(ui, Event::Error(format!("No se pudo enviar: {e}")));
            }
        }
    }
}

async fn open(target: &Target) -> Result<Conn, String> {
    let api = StreamApi::new();
    let (rx, api) = match target {
        Target::Ble { mac, .. } => {
            let id = BleId::from_mac_address(mac).map_err(|e| e.to_string())?;
            let s = utils::stream::build_ble_stream(&id, Duration::from_secs(8))
                .await
                .map_err(|e| format!("Bluetooth: {e}"))?;
            api.connect(s).await
        }
        Target::Serial { port } => {
            let s = utils::stream::build_serial_stream(port.clone(), None, None, None)
                .map_err(|e| format!("USB: {e}"))?;
            api.connect(s).await
        }
        Target::Tcp { addr } => {
            let addr = if addr.contains(':') {
                addr.clone()
            } else {
                format!("{addr}:4403")
            };
            let s = utils::stream::build_tcp_stream(addr)
                .await
                .map_err(|e| format!("Red: {e}"))?;
            api.connect(s).await
        }
    };
    let api = api
        .configure(utils::generate_rand_id())
        .await
        .map_err(|e| e.to_string())?;
    Ok(Conn { api, rx })
}

fn handle_from_radio(fr: FromRadio, router: &mut Router, ui: &Ui) {
    let Some(v) = fr.payload_variant else { return };
    match v {
        from_radio::PayloadVariant::MyInfo(m) => {
            router.my = m.my_node_num;
            emit(ui, Event::MyNode(m.my_node_num));
        }
        from_radio::PayloadVariant::NodeInfo(n) => emit(ui, Event::Node(node_from_info(n))),
        from_radio::PayloadVariant::Channel(c) => {
            if c.role != 0 {
                emit(
                    ui,
                    Event::Channel(ChannelInfo {
                        index: c.index as u32,
                        name: c.settings.map(|s| s.name).unwrap_or_default(),
                        role: c.role,
                    }),
                );
            }
        }
        from_radio::PayloadVariant::ConfigCompleteId(_) => emit(ui, Event::ConfigComplete),
        from_radio::PayloadVariant::Packet(p) => handle_mesh(p, router.my, ui),
        _ => {}
    }
}

fn handle_mesh(p: MeshPacket, my: u32, ui: &Ui) {
    let mut patch = NodePatch {
        heard_at: Some(if p.rx_time > 0 { p.rx_time } else { now() }),
        snr: (p.rx_snr != 0.0).then_some(p.rx_snr),
        rssi: (p.rx_rssi != 0).then_some(p.rx_rssi),
        hops: (p.hop_start > 0).then(|| p.hop_start.saturating_sub(p.hop_limit)),
        ..Default::default()
    };

    if let Some(mesh_packet::PayloadVariant::Decoded(d)) = &p.payload_variant {
        match PortNum::try_from(d.portnum) {
            Ok(PortNum::TextMessageApp) if p.from != my => {
                let conv = if p.to == my && p.to != BROADCAST {
                    Conv::Direct(p.from)
                } else {
                    Conv::Channel(p.channel)
                };
                emit(
                    ui,
                    Event::Message(Msg {
                        id: p.id,
                        conv,
                        from: p.from,
                        text: String::from_utf8_lossy(&d.payload).into_owned(),
                        time: if p.rx_time > 0 { p.rx_time } else { now() },
                        delivery: Delivery::Incoming,
                        error: None,
                        snr: patch.snr,
                        rssi: patch.rssi,
                        hops: patch.hops,
                    }),
                );
            }
            Ok(PortNum::NodeinfoApp) => {
                if let Ok(u) = protobufs::User::decode(d.payload.as_slice()) {
                    patch.user = Some((
                        u.long_name,
                        u.short_name,
                        hw_name(u.hw_model),
                        role_name(u.role),
                        !u.public_key.is_empty(),
                    ));
                }
            }
            Ok(PortNum::PositionApp) => {
                if let Ok(pos) = protobufs::Position::decode(d.payload.as_slice()) {
                    if let (Some(la), Some(lo)) = (pos.latitude_i, pos.longitude_i) {
                        if la != 0 || lo != 0 {
                            patch.position =
                                Some((la as f64 * 1e-7, lo as f64 * 1e-7, pos.altitude));
                        }
                    }
                }
            }
            Ok(PortNum::TelemetryApp) => {
                if let Ok(t) = protobufs::Telemetry::decode(d.payload.as_slice()) {
                    if let Some(telemetry::Variant::DeviceMetrics(m)) = t.variant {
                        patch.battery = m.battery_level;
                        patch.voltage = m.voltage;
                        patch.channel_util = m.channel_utilization;
                        patch.air_util_tx = m.air_util_tx;
                        patch.uptime = m.uptime_seconds;
                    }
                }
            }
            Ok(PortNum::RoutingApp) if d.request_id != 0 => {
                if let Ok(r) = protobufs::Routing::decode(d.payload.as_slice()) {
                    if let Some(routing::Variant::ErrorReason(code)) = r.variant {
                        emit(
                            ui,
                            Event::Delivery {
                                id: d.request_id,
                                ok: code == 0,
                                error: (code != 0).then(|| routing_error(code)),
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }

    if p.from != 0 {
        emit(ui, Event::NodePatch(p.from, patch));
    }
}

fn node_from_info(n: protobufs::NodeInfo) -> Node {
    let mut node = Node {
        num: n.num,
        snr: (n.snr != 0.0).then_some(n.snr),
        last_heard: n.last_heard,
        hops_away: n.hops_away,
        via_mqtt: n.via_mqtt,
        favorite: n.is_favorite,
        ..Default::default()
    };
    if let Some(u) = n.user {
        node.long_name = u.long_name;
        node.short_name = u.short_name;
        node.hw_model = hw_name(u.hw_model);
        node.role = role_name(u.role);
        node.has_pubkey = !u.public_key.is_empty();
    }
    if let Some(m) = n.device_metrics {
        node.battery = m.battery_level;
        node.voltage = m.voltage;
        node.channel_util = m.channel_utilization;
        node.air_util_tx = m.air_util_tx;
        node.uptime = m.uptime_seconds;
    }
    if let Some(pos) = n.position {
        if let (Some(la), Some(lo)) = (pos.latitude_i, pos.longitude_i) {
            if la != 0 || lo != 0 {
                node.lat = Some(la as f64 * 1e-7);
                node.lon = Some(lo as f64 * 1e-7);
                node.alt = pos.altitude;
            }
        }
    }
    node
}

fn hw_name(v: i32) -> String {
    protobufs::HardwareModel::try_from(v)
        .map(|h| h.as_str_name().to_string())
        .unwrap_or_else(|_| format!("modelo {v}"))
}

fn role_name(v: i32) -> String {
    protobufs::config::device_config::Role::try_from(v)
        .map(|r| r.as_str_name().to_string())
        .unwrap_or_default()
}

fn routing_error(code: i32) -> String {
    use routing::Error as E;
    match E::try_from(code) {
        Ok(E::MaxRetransmit) => "Nadie confirmó la recepción".into(),
        Ok(E::NoRoute) => "No hay ruta hasta el destino".into(),
        Ok(E::Timeout) => "Tiempo agotado".into(),
        Ok(E::TooLarge) => "Mensaje demasiado largo".into(),
        Ok(E::NoChannel) => "Canal no válido".into(),
        Ok(E::DutyCycleLimit) => "Límite legal de uso del aire alcanzado; espera un poco".into(),
        Ok(E::PkiFailed) | Ok(E::PkiUnknownPubkey) => {
            "Fallo de cifrado directo (falta la clave del destino)".into()
        }
        Ok(e) => e.as_str_name().to_string(),
        Err(_) => format!("Error {code}"),
    }
}
