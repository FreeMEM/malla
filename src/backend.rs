//! Hilo de radio: habla con el nodo Meshtastic usando tokio y avisa a la UI.

use std::convert::Infallible;
use std::time::Duration;

use meshtastic::api::{ConnectedStreamApi, StreamApi};
use meshtastic::packet::{PacketDestination, PacketReceiver, PacketRouter};
use meshtastic::protobufs::{
    self, admin_message, from_radio, mesh_packet, routing, telemetry, FromRadio, MeshPacket, PortNum,
};
use meshtastic::types::{MeshChannel, NodeId};
use meshtastic::utils;
use meshtastic::utils::stream::BleId;
use meshtastic::Message as _;
use tokio::sync::mpsc;
use tokio::time::Instant;

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
                let conv = if p.to == BROADCAST { Conv::Channel(p.channel) } else { Conv::Direct(p.to) };
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
                        reply_id: (d.reply_id != 0).then_some(d.reply_id),
                        emoji: d.emoji != 0,
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
    Retry,
}

struct Radio {
    conn: Option<Conn>,
    router: Router,
    ui: Ui,
    /// Conexión que el usuario quiere mantener (para reconectar sola).
    wanted: Option<Target>,
    retry_at: Option<Instant>,
    retries: u32,
    my_user: Option<protobufs::User>,
}

async fn run(mut cmds: mpsc::UnboundedReceiver<Command>, ui: Ui) {
    let mut r = Radio {
        conn: None,
        router: Router { my: 0, ui: ui.clone() },
        ui,
        wanted: None,
        retry_at: None,
        retries: 0,
        my_user: None,
    };

    loop {
        let retry_at = r.retry_at;
        let step = match r.conn.as_mut() {
            Some(c) => tokio::select! {
                cmd = cmds.recv() => Step::Cmd(cmd),
                pkt = c.rx.recv() => Step::Pkt(pkt),
            },
            None => match retry_at {
                Some(at) => tokio::select! {
                    cmd = cmds.recv() => Step::Cmd(cmd),
                    _ = tokio::time::sleep_until(at) => Step::Retry,
                },
                None => Step::Cmd(cmds.recv().await),
            },
        };

        match step {
            Step::Cmd(None) => break,
            Step::Cmd(Some(cmd)) => r.handle_cmd(cmd).await,
            Step::Pkt(Some(p)) => r.handle_from_radio(p),
            Step::Pkt(None) => {
                r.conn = None;
                r.schedule_retry("Se perdió la conexión con el nodo".into());
            }
            Step::Retry => {
                r.retry_at = None;
                if let Some(t) = r.wanted.clone() {
                    r.connect(t).await;
                }
            }
        }
    }

    if let Some(c) = r.conn {
        let _ = c.api.disconnect().await;
    }
}

impl Radio {
    fn schedule_retry(&mut self, reason: String) {
        match self.wanted.clone() {
            Some(target) => {
                self.retries += 1;
                let secs = (5 * self.retries as u64).min(30);
                self.retry_at = Some(Instant::now() + Duration::from_secs(secs));
                emit(&self.ui, Event::Reconnecting { target, secs, reason });
            }
            None => emit(&self.ui, Event::Disconnected(Some(reason))),
        }
    }

    async fn connect(&mut self, target: Target) {
        if let Some(c) = self.conn.take() {
            let _ = c.api.disconnect().await;
        }
        self.router.my = 0;
        emit(&self.ui, Event::Connecting(target.clone()));
        match tokio::time::timeout(Duration::from_secs(40), open(&target)).await {
            Ok(Ok(c)) => {
                self.conn = Some(c);
                self.retries = 0;
                self.retry_at = None;
                emit(&self.ui, Event::Connected(target));
            }
            Ok(Err(e)) => self.schedule_retry(e),
            Err(_) => self.schedule_retry("El nodo no respondió a tiempo".into()),
        }
    }

    async fn handle_cmd(&mut self, cmd: Command) {
        match cmd {
            Command::ScanBle => {
                let ui = self.ui.clone();
                tokio::spawn(async move {
                    let res = utils::stream::available_ble_devices(Duration::from_secs(6))
                        .await
                        .map(|v| {
                            v.into_iter()
                                .map(|d| (d.name.unwrap_or_else(|| "(sin nombre)".into()), d.mac_address.to_string()))
                                .collect()
                        })
                        .map_err(|e| e.to_string());
                    emit(&ui, Event::BleDevices(res));
                });
            }
            Command::Connect(target) => {
                self.wanted = Some(target.clone());
                self.retries = 0;
                self.retry_at = None;
                self.connect(target).await;
            }
            Command::Disconnect => {
                self.wanted = None;
                self.retry_at = None;
                if let Some(c) = self.conn.take() {
                    let _ = c.api.disconnect().await;
                }
                emit(&self.ui, Event::Disconnected(None));
            }
            Command::SendText { conv, text, reply_id, emoji } => {
                let (dest, ch) = dest_of(conv);
                let data = text.into_bytes();
                self.send(data, PortNum::TextMessageApp, dest, ch, true, false, true, reply_id, emoji.then_some(1))
                    .await;
            }
            Command::Traceroute(n) => {
                let data = protobufs::RouteDiscovery::default().encode_to_vec();
                if self
                    .send(data, PortNum::TracerouteApp, PacketDestination::Node(NodeId::new(n)), 0, false, true, false, None, None)
                    .await
                {
                    emit(&self.ui, Event::Info("Traceroute enviado. La respuesta puede tardar un minuto.".into()));
                }
            }
            Command::RequestPosition(n) => {
                let data = protobufs::Position { time: now(), ..Default::default() }.encode_to_vec();
                if self
                    .send(data, PortNum::PositionApp, PacketDestination::Node(NodeId::new(n)), 0, false, true, false, None, None)
                    .await
                {
                    emit(&self.ui, Event::Info("Posición pedida.".into()));
                }
            }
            Command::RequestUser(n) => {
                let Some(u) = self.my_user.clone() else {
                    emit(&self.ui, Event::Error("Aún no conozco los datos de tu nodo".into()));
                    return;
                };
                if self
                    .send(u.encode_to_vec(), PortNum::NodeinfoApp, PacketDestination::Node(NodeId::new(n)), 0, false, true, false, None, None)
                    .await
                {
                    emit(&self.ui, Event::Info("Datos de usuario intercambiados.".into()));
                }
            }
            Command::SendWaypoint(w) => {
                self.send(w.encode_to_vec(), PortNum::WaypointApp, PacketDestination::Broadcast, 0, false, false, false, None, None)
                    .await;
            }
            Command::Admin(list) => self.admin(list).await,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn send(
        &mut self,
        data: Vec<u8>,
        port: PortNum,
        dest: PacketDestination,
        ch: u32,
        want_ack: bool,
        want_response: bool,
        echo: bool,
        reply_id: Option<u32>,
        emoji: Option<u32>,
    ) -> bool {
        let Some(c) = self.conn.as_mut() else {
            emit(&self.ui, Event::Error("No hay conexión con el nodo".into()));
            return false;
        };
        match c
            .api
            .send_mesh_packet(
                &mut self.router,
                data.into(),
                port,
                dest,
                MeshChannel::from(ch),
                want_ack,
                want_response,
                echo,
                reply_id,
                emoji,
            )
            .await
        {
            Ok(()) => true,
            Err(e) => {
                emit(&self.ui, Event::Error(format!("No se pudo enviar: {e}")));
                false
            }
        }
    }

    async fn admin(&mut self, list: Vec<admin_message::PayloadVariant>) {
        use admin_message::PayloadVariant as P;
        let edits = list.iter().any(|p| {
            matches!(p, P::SetConfig(_) | P::SetModuleConfig(_) | P::SetChannel(_) | P::SetOwner(_) | P::SetFixedPosition(_))
        });
        let mut all = vec![];
        if edits {
            all.push(P::BeginEditSettings(true));
        }
        all.extend(list);
        if edits {
            all.push(P::CommitEditSettings(true));
        }
        for p in all {
            let msg = protobufs::AdminMessage { payload_variant: Some(p), session_passkey: vec![] };
            if !self
                .send(msg.encode_to_vec(), PortNum::AdminApp, PacketDestination::Local, 0, true, false, false, None, None)
                .await
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        if edits {
            emit(&self.ui, Event::Info("Cambios enviados al nodo. Puede que se reinicie.".into()));
        }
    }

    fn handle_from_radio(&mut self, fr: FromRadio) {
        let Some(v) = fr.payload_variant else { return };
        let ui = &self.ui;
        match v {
            from_radio::PayloadVariant::MyInfo(m) => {
                self.router.my = m.my_node_num;
                emit(ui, Event::MyNode(m.my_node_num));
            }
            from_radio::PayloadVariant::NodeInfo(n) => {
                if n.num == self.router.my {
                    if let Some(u) = &n.user {
                        self.my_user = Some(u.clone());
                        emit(ui, Event::MyUser(u.clone()));
                    }
                }
                emit(ui, Event::Node(node_from_info(n)));
            }
            from_radio::PayloadVariant::Channel(c) => {
                if c.role != 0 {
                    emit(
                        ui,
                        Event::Channel(ChannelInfo {
                            index: c.index as u32,
                            name: c.settings.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
                            role: c.role,
                        }),
                    );
                }
                emit(ui, Event::ChannelFull(c));
            }
            from_radio::PayloadVariant::Config(c) => emit(ui, Event::Config(c)),
            from_radio::PayloadVariant::ModuleConfig(c) => emit(ui, Event::ModuleConfig(c)),
            from_radio::PayloadVariant::Metadata(m) => emit(ui, Event::Metadata(m)),
            from_radio::PayloadVariant::ConfigCompleteId(_) => emit(ui, Event::ConfigComplete),
            from_radio::PayloadVariant::Rebooted(_) => emit(ui, Event::Info("El nodo se ha reiniciado".into())),
            from_radio::PayloadVariant::Packet(p) => self.handle_mesh(p),
            _ => {}
        }
    }

    fn handle_mesh(&mut self, p: MeshPacket) {
        let my = self.router.my;
        let ui = &self.ui;
        let mut patch = NodePatch {
            heard_at: Some(if p.rx_time > 0 { p.rx_time } else { now() }),
            snr: (p.rx_snr != 0.0).then_some(p.rx_snr),
            rssi: (p.rx_rssi != 0).then_some(p.rx_rssi),
            hops: (p.hop_start > 0).then(|| p.hop_start.saturating_sub(p.hop_limit)),
            via_mqtt: Some(p.via_mqtt),
            ..Default::default()
        };
        let mut log = PacketLog {
            time: patch.heard_at.unwrap_or(0),
            from: p.from,
            to: p.to,
            port: "cifrado".into(),
            channel: p.channel,
            snr: patch.snr,
            rssi: patch.rssi,
            hops: patch.hops,
            size: 0,
            encrypted: true,
            via_mqtt: p.via_mqtt,
            summary: String::new(),
        };

        if let Some(mesh_packet::PayloadVariant::Decoded(d)) = &p.payload_variant {
            log.encrypted = false;
            log.size = d.payload.len();
            let port = PortNum::try_from(d.portnum);
            log.port = port.map(|p| p.as_str_name().to_string()).unwrap_or_else(|_| format!("puerto {}", d.portnum));
            match port {
                Ok(PortNum::TextMessageApp) if p.from != my => {
                    let conv = if p.to == my && p.to != BROADCAST { Conv::Direct(p.from) } else { Conv::Channel(p.channel) };
                    let text = String::from_utf8_lossy(&d.payload).into_owned();
                    log.summary = text.chars().take(60).collect();
                    emit(
                        ui,
                        Event::Message(Msg {
                            id: p.id,
                            conv,
                            from: p.from,
                            text,
                            time: if p.rx_time > 0 { p.rx_time } else { now() },
                            delivery: Delivery::Incoming,
                            error: None,
                            snr: patch.snr,
                            rssi: patch.rssi,
                            hops: patch.hops,
                            reply_id: (d.reply_id != 0).then_some(d.reply_id),
                            emoji: d.emoji != 0,
                        }),
                    );
                }
                Ok(PortNum::NodeinfoApp) => {
                    if let Ok(u) = protobufs::User::decode(d.payload.as_slice()) {
                        log.summary = format!("{} ({})", u.long_name, u.short_name);
                        patch.user = Some(u);
                    }
                }
                Ok(PortNum::PositionApp) => {
                    if let Ok(pos) = protobufs::Position::decode(d.payload.as_slice()) {
                        if let (Some(la), Some(lo)) = (pos.latitude_i, pos.longitude_i) {
                            if la != 0 || lo != 0 {
                                let (la, lo) = (la as f64 * 1e-7, lo as f64 * 1e-7);
                                log.summary = format!("{la:.5}, {lo:.5}");
                                patch.position = Some((la, lo, pos.altitude));
                            }
                        }
                    }
                }
                Ok(PortNum::TelemetryApp) => {
                    if let Ok(t) = protobufs::Telemetry::decode(d.payload.as_slice()) {
                        match t.variant {
                            Some(telemetry::Variant::DeviceMetrics(m)) => {
                                patch.battery = m.battery_level;
                                patch.voltage = m.voltage;
                                patch.channel_util = m.channel_utilization;
                                patch.air_util_tx = m.air_util_tx;
                                patch.uptime = m.uptime_seconds;
                                log.summary = "métricas del aparato".into();
                            }
                            Some(telemetry::Variant::EnvironmentMetrics(m)) => {
                                patch.temperature = m.temperature;
                                patch.humidity = m.relative_humidity;
                                patch.pressure = m.barometric_pressure;
                                log.summary = "entorno".into();
                            }
                            Some(_) => log.summary = "otra telemetría".into(),
                            None => {}
                        }
                    }
                }
                Ok(PortNum::RoutingApp) if d.request_id != 0 => {
                    if let Ok(r) = protobufs::Routing::decode(d.payload.as_slice()) {
                        if let Some(routing::Variant::ErrorReason(code)) = r.variant {
                            log.summary = if code == 0 { "confirmación".into() } else { routing_error(code) };
                            emit(ui, Event::Delivery { id: d.request_id, ok: code == 0, error: (code != 0).then(|| routing_error(code)) });
                        }
                    }
                }
                Ok(PortNum::TracerouteApp) if p.to == my && d.request_id != 0 => {
                    if let Ok(rd) = protobufs::RouteDiscovery::decode(d.payload.as_slice()) {
                        let snr = |v: &Vec<i32>| v.iter().map(|s| (*s != i8::MIN as i32).then(|| *s as f32 / 4.0)).collect();
                        log.summary = format!("ruta de {} saltos", rd.route.len() + 1);
                        emit(
                            ui,
                            Event::Trace(Trace {
                                target: p.from,
                                time: now(),
                                towards: rd.route.clone(),
                                snr_towards: snr(&rd.snr_towards),
                                back: rd.route_back.clone(),
                                snr_back: snr(&rd.snr_back),
                                has_back: !rd.snr_back.is_empty(),
                            }),
                        );
                    }
                }
                Ok(PortNum::WaypointApp) => {
                    if let Ok(w) = protobufs::Waypoint::decode(d.payload.as_slice()) {
                        log.summary = w.name.clone();
                        emit(
                            ui,
                            Event::Waypoint(Waypoint {
                                id: w.id,
                                lat: w.latitude_i.unwrap_or(0) as f64 * 1e-7,
                                lon: w.longitude_i.unwrap_or(0) as f64 * 1e-7,
                                name: w.name,
                                description: w.description,
                                icon: w.icon,
                                expire: w.expire,
                                locked_to: w.locked_to,
                                from: p.from,
                                time: now(),
                            }),
                        );
                    }
                }
                _ => {}
            }
        }

        emit(ui, Event::Packet(log));
        if p.from != 0 {
            emit(ui, Event::NodePatch(p.from, patch));
        }
    }
}

fn dest_of(conv: Conv) -> (PacketDestination, u32) {
    match conv {
        Conv::Channel(ch) => (PacketDestination::Broadcast, ch),
        Conv::Direct(n) => (PacketDestination::Node(NodeId::new(n)), 0),
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
            let s = utils::stream::build_serial_stream(port.clone(), None, None, None).map_err(|e| format!("USB: {e}"))?;
            api.connect(s).await
        }
        Target::Tcp { addr } => {
            let addr = if addr.contains(':') { addr.clone() } else { format!("{addr}:4403") };
            let s = utils::stream::build_tcp_stream(addr).await.map_err(|e| format!("Red: {e}"))?;
            api.connect(s).await
        }
    };
    let api = api.configure(utils::generate_rand_id()).await.map_err(|e| e.to_string())?;
    Ok(Conn { api, rx })
}

fn node_from_info(n: protobufs::NodeInfo) -> Node {
    let mut node = Node {
        num: n.num,
        snr: (n.snr != 0.0).then_some(n.snr),
        last_heard: n.last_heard,
        hops_away: n.hops_away,
        via_mqtt: n.via_mqtt,
        favorite: n.is_favorite,
        ignored: n.is_ignored,
        ..Default::default()
    };
    if let Some(u) = n.user {
        node.long_name = u.long_name;
        node.short_name = u.short_name;
        node.hw_model = hw_name(u.hw_model);
        node.role = role_name(u.role);
        node.has_pubkey = !u.public_key.is_empty();
        node.licensed = u.is_licensed;
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

fn routing_error(code: i32) -> String {
    use routing::Error as E;
    match E::try_from(code) {
        Ok(E::MaxRetransmit) => "Nadie confirmó la recepción".into(),
        Ok(E::NoRoute) => "No hay ruta hasta el destino".into(),
        Ok(E::Timeout) => "Tiempo agotado".into(),
        Ok(E::TooLarge) => "Mensaje demasiado largo".into(),
        Ok(E::NoChannel) => "Canal no válido".into(),
        Ok(E::NoResponse) => "El destino no respondió".into(),
        Ok(E::DutyCycleLimit) => "Límite legal de uso del aire alcanzado; espera un poco".into(),
        Ok(E::RateLimitExceeded) => "Demasiados mensajes seguidos; espera un poco".into(),
        Ok(E::PkiFailed) | Ok(E::PkiUnknownPubkey) | Ok(E::PkiSendFailPublicKey) => {
            "Fallo de cifrado directo (falta la clave del destino)".into()
        }
        Ok(e) => e.as_str_name().to_string(),
        Err(_) => format!("Error {code}"),
    }
}
