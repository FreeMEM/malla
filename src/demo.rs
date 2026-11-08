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
        for m in mensajes(t) {
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
                Command::SendText { conv, text } => {
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
                    }));
                    std::thread::sleep(Duration::from_millis(1500));
                    emit(Event::Delivery { id: siguiente, ok: true, error: None });
                }
                Command::Connect(_) | Command::Disconnect => {}
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
        m(8, cs, 0x13cd_4e05, 1200, "Temperatura 18 °C, humedad 64 %, viento flojo.", Incoming, Some(1), Some(1.5)),
        m(9, cs, 0x64de_5f06, 400, "Paneles al 92 %, todo en orden.", Incoming, Some(2), Some(-9.25)),
        m(10, mira, 0x9f21_0a11, 2000, "Oye, ¿qué antena llevas? Te oigo muy fuerte.", Incoming, Some(0), Some(7.25)),
        m(11, mira, YO, 1900, "Una colineal de 5 dBi en el tejado.", Acked, None, None),
    ]
}
