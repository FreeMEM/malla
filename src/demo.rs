//! `malla --demo`: datos inventados, sin radio y sin guardar nada.
//! Sirve para las capturas del README y para probar la interfaz.

use std::time::Duration;

use tokio::sync::mpsc;

use crate::model::*;

const YO: u32 = 0x1a2b_3c4d;

pub fn spawn(ui: async_channel::Sender<Event>) -> mpsc::UnboundedSender<Command> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Command>();
    std::thread::spawn(move || {
        let emit = |e: Event| {
            let _ = ui.send_blocking(e);
        };
        let destino = Target::Ble { name: "Meshtastic_3c4d".into(), mac: "AA:BB:CC:DD:3C:4D".into() };
        emit(Event::Connecting(destino.clone()));
        std::thread::sleep(Duration::from_millis(400));
        emit(Event::Connected(destino));
        emit(Event::MyNode(YO));
        for (i, name, role) in [(0, "LongFast", 1), (1, "CostaSol", 2), (2, "Pruebas", 2)] {
            emit(Event::Channel(ChannelInfo { index: i, name: name.into(), role }));
        }
        let t = now();
        for n in nodos(t) {
            emit(Event::Node(n));
        }
        extra(&emit, t);
        let mut ms = mensajes(t);
        ms.sort_by_key(|m| m.time);
        for m in ms {
            emit(Event::Message(m));
        }
        emit(Event::ConfigComplete);
        {
            let ui = ui.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(10));
                let _ = ui.send_blocking(Event::Message(Msg {
                    id: 99,
                    conv: Conv::Channel(1),
                    from: 0x22be_3d04,
                    text: "Llegando a puerto, nos vemos en el muelle.".into(),
                    time: now(),
                    delivery: Delivery::Incoming,
                    error: None,
                    snr: Some(-5.0),
                    rssi: Some(-120),
                    hops: Some(2),
                    reply_id: None,
                    emoji: false,
                }));
            });
        }

        let mut siguiente = 0xdead_0000u32;
        while let Some(c) = rx.blocking_recv() {
            match c {
                Command::ScanBle => {
                    std::thread::sleep(Duration::from_secs(1));
                    emit(Event::BleDevices(Ok(vec![
                        ("Meshtastic_3c4d".into(), "AA:BB:CC:DD:3C:4D".into()),
                        ("MIRA_9f21".into(), "AA:BB:CC:DD:9F:21".into()),
                    ])));
                }
                Command::SendText { conv, text, reply_id, emoji } => {
                    siguiente += 1;
                    emit(Event::Message(Msg {
                        id: siguiente,
                        conv,
                        from: YO,
                        text,
                        time: now(),
                        delivery: Delivery::Pending,
                        error: None,
                        snr: None,
                        rssi: None,
                        hops: None,
                        reply_id,
                        emoji,
                    }));
                    std::thread::sleep(Duration::from_millis(1500));
                    emit(Event::Delivery { id: siguiente, ok: true, error: None });
                }
                Command::Traceroute(n) => {
                    std::thread::sleep(Duration::from_secs(2));
                    emit(Event::Trace(Trace {
                        target: n,
                        time: now(),
                        towards: vec![0x7c33_1b02],
                        snr_towards: vec![Some(6.25), Some(-3.5)],
                        back: vec![0x7c33_1b02],
                        snr_back: vec![Some(-2.0), Some(5.75)],
                        has_back: true,
                    }));
                }
                Command::Connect(_)
                | Command::Disconnect
                | Command::RequestPosition(_)
                | Command::RequestUser(_)
                | Command::SendWaypoint(_)
                | Command::Admin(_) => {}
            }
        }
    });
    tx
}

#[allow(clippy::too_many_arguments)]
fn nodo(
    num: u32,
    long: &str,
    short: &str,
    hw: &str,
    role: &str,
    hace: u32,
    hops: Option<u32>,
    snr: Option<f32>,
    bat: Option<u32>,
    pos: Option<(f64, f64)>,
    t: u32,
) -> Node {
    Node {
        num,
        long_name: long.into(),
        short_name: short.into(),
        hw_model: hw.into(),
        role: role.into(),
        snr,
        rssi: snr.map(|s| (-110.0 + s * 4.0) as i32),
        last_heard: t.saturating_sub(hace),
        hops_away: hops,
        battery: bat,
        voltage: bat.map(|b| 3.3 + b.min(100) as f32 / 100.0 * 0.9),
        channel_util: Some(8.5),
        air_util_tx: Some(1.2),
        uptime: Some(3 * 86400 + 4 * 3600),
        lat: pos.map(|p| p.0),
        lon: pos.map(|p| p.1),
        alt: pos.map(|_| 120),
        has_pubkey: true,
        ..Default::default()
    }
}

fn nodos(t: u32) -> Vec<Node> {
    vec![
        nodo(YO, "Malla Demo", "MLLA", "HELTEC_V3", "CLIENT", 0, Some(0), None, Some(87), Some((36.541, -4.625)), t),
        nodo(0x9f21_0a11, "Mirador de la Sierra", "MIRA", "HELTEC_V3", "ROUTER", 45, Some(0), Some(7.25), Some(101), Some((36.622, -4.700)), t),
        nodo(0x7c33_1b02, "Torre del Puerto", "TORR", "STATION_G2", "ROUTER", 120, Some(0), Some(5.5), Some(101), Some((36.598, -4.517)), t),
        nodo(0x51aa_2c03, "Repetidor Cerro Alto", "CERR", "RAK4631", "ROUTER", 300, Some(1), Some(-2.75), Some(64), Some((36.672, -4.790)), t),
        nodo(0x22be_3d04, "Velero Brisa", "VELA", "T_ECHO", "CLIENT", 600, Some(2), Some(-6.0), Some(48), Some((36.505, -4.560)), t),
        nodo(0x13cd_4e05, "Estación Meteo Río", "METO", "HELTEC_V4", "SENSOR", 900, Some(1), Some(1.5), Some(101), Some((36.655, -4.612)), t),
        nodo(0x64de_5f06, "Huerto Solar", "SOLR", "XIAO_NRF52_KIT", "CLIENT_MUTE", 1800, Some(2), Some(-9.25), Some(92), Some((36.700, -4.660)), t),
        nodo(0x45ef_6a07, "Bici Ruta Costa", "BICI", "TBEAM", "TRACKER", 3600, Some(3), Some(-12.0), Some(35), Some((36.480, -4.720)), t),
        nodo(0x36f0_7b08, "Faro Norte", "FARO", "HELTEC_V4", "ROUTER", 7200, Some(2), Some(-4.0), Some(101), Some((36.720, -4.420)), t),
        nodo(0x2701_8c09, "Portátil de Prueba", "PRUE", "RAK4631", "CLIENT", 14400, Some(4), Some(-14.5), Some(12), None, t),
        nodo(0x1812_9d10, "", "", "", "", 86400 * 2, None, None, None, None, t),
    ]
}

fn mensajes(t: u32) -> Vec<Msg> {
    let m = |id: u32, conv: Conv, from: u32, hace: u32, text: &str, d: Delivery, hops: Option<u32>, snr: Option<f32>| Msg {
        id,
        conv,
        from,
        text: text.into(),
        time: t.saturating_sub(hace),
        delivery: d,
        error: None,
        snr,
        rssi: snr.map(|s| (-110.0 + s * 4.0) as i32),
        hops,
        reply_id: None,
        emoji: false,
    };
    use Delivery::*;
    let lf = Conv::Channel(0);
    let cs = Conv::Channel(1);
    let mira = Conv::Direct(0x9f21_0a11);
    vec![
        m(1, lf, 0x9f21_0a11, 3000, "Buenos días desde la sierra. ¿Quién me oye?", Incoming, Some(0), Some(7.25)),
        m(2, lf, 0x7c33_1b02, 2900, "Alto y claro desde el puerto, SNR 5 por aquí.", Incoming, Some(0), Some(5.5)),
        m(3, lf, YO, 2700, "Probando Malla, un cliente de escritorio para Meshtastic 🙂", Acked, None, None),
        m(4, lf, 0x22be_3d04, 2400, "Saliendo del puerto con el velero, os mando la posición cada 15 min.", Incoming, Some(2), Some(-6.0)),
        m(5, lf, 0x51aa_2c03, 1500, "El repetidor del cerro ya tiene la batería nueva. Debería aguantar el invierno.", Incoming, Some(1), Some(-2.75)),
        m(6, lf, YO, 600, "¡Genial! Desde la costa os llega bien.", Acked, None, None),
        m(7, lf, YO, 30, "¿Alguien prueba esta noche a las 22:00?", Pending, None, None),
        Msg { reply_id: Some(6), ..m(12, lf, 0x9f21_0a11, 500, "Te llega perfecto, 7 dB de SNR desde aquí arriba", Incoming, Some(0), Some(7.0)) },
        Msg { reply_id: Some(3), emoji: true, ..m(13, lf, 0x7c33_1b02, 2600, "👍", Incoming, Some(0), Some(5.5)) },
        Msg { reply_id: Some(3), emoji: true, ..m(14, lf, 0x51aa_2c03, 2500, "👍", Incoming, Some(1), Some(-2.0)) },
        Msg { reply_id: Some(5), emoji: true, ..m(15, lf, YO, 1400, "🎉", Acked, None, None) },
        m(8, cs, 0x13cd_4e05, 1200, "Temperatura 18 °C, humedad 64 %, viento flojo.", Incoming, Some(1), Some(1.5)),
        m(9, cs, 0x64de_5f06, 400, "Paneles al 92 %, todo en orden.", Incoming, Some(2), Some(-9.25)),
        m(10, mira, 0x9f21_0a11, 2000, "Oye, ¿qué antena llevas? Te oigo muy fuerte.", Incoming, Some(0), Some(7.25)),
        m(11, mira, YO, 1900, "Una colineal de 5 dBi en el tejado.", Acked, None, None),
    ]
}

/// Configuración, canales, telemetría pasada y un rastro, para probar Ajustes y gráficas.
fn extra(emit: &dyn Fn(Event), t: u32) {
    use meshtastic::protobufs::{self as pb, config, module_config};
    emit(Event::MyUser(pb::User {
        id: "!1a2b3c4d".into(),
        long_name: "Malla Demo".into(),
        short_name: "MLLA".into(),
        hw_model: pb::HardwareModel::HeltecV3 as i32,
        ..Default::default()
    }));
    emit(Event::Metadata(pb::DeviceMetadata {
        firmware_version: "2.7.15.demo".into(),
        has_bluetooth: true,
        has_wifi: true,
        can_shutdown: true,
        has_pkc: true,
        hw_model: pb::HardwareModel::HeltecV3 as i32,
        ..Default::default()
    }));
    let cfgs = vec![
        config::PayloadVariant::Device(config::DeviceConfig { node_info_broadcast_secs: 10800, ..Default::default() }),
        config::PayloadVariant::Position(config::PositionConfig { position_broadcast_secs: 900, position_broadcast_smart_enabled: true, ..Default::default() }),
        config::PayloadVariant::Power(config::PowerConfig::default()),
        config::PayloadVariant::Network(config::NetworkConfig { ntp_server: "meshtastic.pool.ntp.org".into(), ..Default::default() }),
        config::PayloadVariant::Display(config::DisplayConfig { screen_on_secs: 60, ..Default::default() }),
        config::PayloadVariant::Lora(config::LoRaConfig {
            use_preset: true,
            region: config::lo_ra_config::RegionCode::Eu868 as i32,
            modem_preset: config::lo_ra_config::ModemPreset::LongFast as i32,
            hop_limit: 3,
            tx_enabled: true,
            ..Default::default()
        }),
        config::PayloadVariant::Bluetooth(config::BluetoothConfig { enabled: true, fixed_pin: 123456, ..Default::default() }),
        config::PayloadVariant::Security(config::SecurityConfig { serial_enabled: true, ..Default::default() }),
    ];
    for c in cfgs {
        emit(Event::Config(pb::Config { payload_variant: Some(c) }));
    }
    let mods = vec![
        module_config::PayloadVariant::Mqtt(module_config::MqttConfig {
            address: "mqtt.meshtastic.org".into(),
            username: "meshdev".into(),
            password: "large4cats".into(),
            root: "msh/EU_868".into(),
            encryption_enabled: true,
            ..Default::default()
        }),
        module_config::PayloadVariant::Telemetry(module_config::TelemetryConfig { device_update_interval: 1800, ..Default::default() }),
        module_config::PayloadVariant::NeighborInfo(module_config::NeighborInfoConfig::default()),
        module_config::PayloadVariant::StoreForward(module_config::StoreForwardConfig::default()),
        module_config::PayloadVariant::RangeTest(module_config::RangeTestConfig::default()),
        module_config::PayloadVariant::CannedMessage(module_config::CannedMessageConfig::default()),
        module_config::PayloadVariant::ExternalNotification(module_config::ExternalNotificationConfig::default()),
        module_config::PayloadVariant::Serial(module_config::SerialConfig::default()),
    ];
    for m in mods {
        emit(Event::ModuleConfig(pb::ModuleConfig { payload_variant: Some(m) }));
    }
    for (i, name, role, psk) in [(0, "", 1, vec![1u8]), (1, "CostaSol", 2, vec![7u8; 32]), (2, "Pruebas", 2, vec![9u8; 16])] {
        emit(Event::ChannelFull(pb::Channel {
            index: i,
            role,
            settings: Some(pb::ChannelSettings { name: name.into(), psk, ..Default::default() }),
        }));
    }
    // 24 h de telemetría del Mirador y de la estación meteo.
    for k in 0..48u32 {
        let tt = t - (48 - k) * 1800;
        let f = k as f32;
        emit(Event::NodePatch(0x9f21_0a11, NodePatch {
            heard_at: Some(tt),
            battery: Some((95.0 - f * 0.8) as u32),
            voltage: Some(4.15 - f * 0.01),
            channel_util: Some(6.0 + (f / 3.0).sin() * 3.0),
            air_util_tx: Some(1.0 + (f / 5.0).cos() * 0.5),
            snr: Some(6.0 + (f / 4.0).sin() * 2.0),
            ..Default::default()
        }));
        emit(Event::NodePatch(0x13cd_4e05, NodePatch {
            heard_at: Some(tt),
            temperature: Some(14.0 + (f / 7.6).sin() * 5.0),
            humidity: Some(60.0 + (f / 7.6).cos() * 12.0),
            pressure: Some(1015.0 + f * 0.1),
            ..Default::default()
        }));
    }
    // Rastro del velero.
    for k in 0..12u32 {
        let f = k as f64;
        emit(Event::NodePatch(0x22be_3d04, NodePatch {
            heard_at: Some(t - (12 - k) * 900),
            position: Some((36.535 - f * 0.003, -4.62 + f * 0.006, None)),
            ..Default::default()
        }));
    }
    let pk = [
        (0x9f21_0a11, BROADCAST, "TEXT_MESSAGE_APP", "Buenos días desde la sierra"),
        (0x7c33_1b02, BROADCAST, "POSITION_APP", "36.59800, -4.51700"),
        (0x13cd_4e05, BROADCAST, "TELEMETRY_APP", "entorno"),
        (0x51aa_2c03, BROADCAST, "NODEINFO_APP", "Repetidor Cerro Alto (CERR)"),
        (0x22be_3d04, YO, "ROUTING_APP", "confirmación"),
        (0x64de_5f06, BROADCAST, "cifrado", ""),
        (0x45ef_6a07, BROADCAST, "TELEMETRY_APP", "métricas del aparato"),
        (0x36f0_7b08, BROADCAST, "NEIGHBORINFO_APP", ""),
    ];
    for (i, (from, to, port, sum)) in pk.iter().enumerate() {
        emit(Event::Packet(PacketLog {
            time: t - (pk.len() - i) as u32 * 37,
            from: *from,
            to: *to,
            port: port.to_string(),
            channel: 0,
            snr: Some(5.0 - i as f32 * 1.7),
            rssi: Some(-80 - i as i32 * 6),
            hops: Some(i as u32 % 3),
            size: if *port == "cifrado" { 0 } else { 20 + i * 7 },
            encrypted: *port == "cifrado",
            via_mqtt: false,
            summary: sum.to_string(),
            detail: serde_json::to_string_pretty(&serde_json::json!({
                "from": from, "to": to, "channel": 0, "id": 0x5a5a_0000u32 + i as u32,
                "rx_time": t - (pk.len() - i) as u32 * 37, "rx_snr": 5.0 - i as f32 * 1.7,
                "rx_rssi": -80 - i as i32 * 6, "hop_limit": 3 - i as u32 % 3, "hop_start": 3,
                "decoded": { "portnum": port, "contenido": sum },
            }))
            .unwrap_or_default(),
        }));
    }
    emit(Event::LocalStats(meshtastic::protobufs::LocalStats {
        uptime_seconds: 3 * 86400 + 5 * 3600,
        channel_utilization: 12.4,
        air_util_tx: 1.8,
        num_packets_tx: 412,
        num_packets_rx: 5310,
        num_packets_rx_bad: 37,
        num_online_nodes: 14,
        num_total_nodes: 23,
        num_rx_dupe: 1890,
        num_tx_relay: 268,
        num_tx_relay_canceled: 95,
        num_tx_dropped: 2,
        ..Default::default()
    }));
    emit(Event::Waypoint(Waypoint {
        id: 4242,
        lat: 36.60,
        lon: -4.58,
        name: "Quedada".into(),
        description: "Sábado 10:00, junto al faro".into(),
        icon: '⛺' as u32,
        expire: 0,
        locked_to: 0,
        from: 0x7c33_1b02,
        time: t,
    }));
}
