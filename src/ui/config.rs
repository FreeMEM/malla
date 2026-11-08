//! Editor genérico de la configuración del nodo y de sus módulos.
//! Cada sección se pasa a JSON, se pinta un control por campo y al guardar
//! se vuelve a convertir al tipo protobuf.

use serde_json::{Map, Value};
use strum::IntoEnumIterator;

use super::*;
use protobufs::config as cfg;
use protobufs::module_config as mcfg;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Kind {
    Radio,
    Module,
}

/// Secciones de radio, en orden: (clave, título, descripción).
pub(super) const RADIO: [(&str, &str, &str); 8] = [
    ("device", "Dispositivo", "Función del nodo, retransmisión y botones"),
    ("position", "Posición", "GPS y cada cuánto se envía la posición"),
    ("power", "Energía", "Ahorro de batería y apagado de pantalla"),
    ("network", "Red", "WiFi, Ethernet y NTP"),
    ("display", "Pantalla", "Unidades, orientación y tiempo encendida"),
    ("lora", "LoRa", "Región, preajuste, saltos y potencia"),
    ("bluetooth", "Bluetooth", "Modo de emparejado y PIN"),
    ("security", "Seguridad", "Administración remota y registros"),
];

pub(super) const MODULES: [(&str, &str, &str); 15] = [
    ("mqtt", "MQTT", "Puente con Internet y mapa público"),
    ("telemetry", "Telemetría", "Métricas del aparato y sensores de entorno"),
    ("neighbor_info", "Info de vecinos", "Anunciar qué nodos oyes directamente"),
    ("store_forward", "Guardar y reenviar", "Guardar mensajes para quien se conecta más tarde"),
    ("range_test", "Prueba de alcance", "Enviar mensajes de prueba periódicos"),
    ("canned_message", "Mensajes predefinidos", "Mensajes rápidos desde el propio aparato"),
    ("external_notification", "Notificación externa", "LED, zumbador o vibración al recibir"),
    ("serial", "Puerto serie", "Texto por el puerto serie del aparato"),
    ("audio", "Audio", "Voz digital Codec2 (solo algunos aparatos)"),
    ("remote_hardware", "Hardware remoto", "Leer y escribir pines desde otros nodos"),
    ("ambient_lighting", "Iluminación", "LED de ambiente"),
    ("detection_sensor", "Sensor de detección", "Avisar cuando cambia un pin"),
    ("paxcounter", "Contador de personas", "Contar aparatos WiFi y Bluetooth cercanos"),
    ("statusmessage", "Mensaje de estado", "Texto de estado del nodo"),
    ("traffic_management", "Gestión de tráfico", "Limitar tráfico en redes muy ocupadas"),
];

/// Campos que no se enseñan: peligrosos o que no se editan como texto.
const HIDDEN: [&str; 6] = ["is_managed", "private_key", "public_key", "admin_key", "session_passkey", "ignore_incoming"];

/// Nombre de la variante de un Config/ModuleConfig ("device", "mqtt"…).
pub(super) fn variant_key<T: serde::Serialize>(c: &T) -> Option<String> {
    let v = serde_json::to_value(c).ok()?;
    v.get("payload_variant")?.as_object()?.keys().next().cloned()
}

fn section_value<T: serde::Serialize>(c: &T) -> Option<(String, Value)> {
    let v = serde_json::to_value(c).ok()?;
    let (k, inner) = v.get("payload_variant")?.as_object()?.iter().next()?;
    Some((k.clone(), inner.clone()))
}

macro_rules! opts {
    ($t:ty) => {
        <$t>::iter().map(|e| (e as i32, e.as_str_name().to_string())).collect::<Vec<(i32, String)>>()
    };
}

fn enum_options(section: &str, field: &str) -> Option<Vec<(i32, String)>> {
    Some(match (section, field) {
        ("device", "role") => opts!(cfg::device_config::Role),
        ("device", "rebroadcast_mode") => opts!(cfg::device_config::RebroadcastMode),
        ("device", "buzzer_mode") => opts!(cfg::device_config::BuzzerMode),
        ("position", "gps_mode") => opts!(cfg::position_config::GpsMode),
        ("network", "address_mode") => opts!(cfg::network_config::AddressMode),
        ("display", "gps_format") => opts!(cfg::display_config::DeprecatedGpsCoordinateFormat),
        ("display", "units") => opts!(cfg::display_config::DisplayUnits),
        ("display", "oled") => opts!(cfg::display_config::OledType),
        ("display", "displaymode") => opts!(cfg::display_config::DisplayMode),
        ("display", "compass_orientation") => opts!(cfg::display_config::CompassOrientation),
        ("lora", "region") => opts!(cfg::lo_ra_config::RegionCode),
        ("lora", "modem_preset") => opts!(cfg::lo_ra_config::ModemPreset),
        ("lora", "fem_lna_mode") => opts!(cfg::lo_ra_config::FemLnaMode),
        ("bluetooth", "mode") => opts!(cfg::bluetooth_config::PairingMode),
        ("serial", "baud") => opts!(mcfg::serial_config::SerialBaud),
        ("serial", "mode") => opts!(mcfg::serial_config::SerialMode),
        ("audio", "bitrate") => opts!(mcfg::audio_config::AudioBaud),
        ("detection_sensor", "detection_trigger_type") => opts!(mcfg::detection_sensor_config::TriggerType),
        ("canned_message", f) if f.starts_with("inputbroker_event") => opts!(mcfg::canned_message_config::InputEventChar),
        _ => return None,
    })
}

/// Nombres en castellano de los campos más comunes.
fn label(field: &str) -> String {
    let s = match field {
        "role" => "Función",
        "rebroadcast_mode" => "Modo de retransmisión",
        "node_info_broadcast_secs" => "Anunciar el nodo cada (s)",
        "double_tap_as_button_press" => "Doble toque como pulsación",
        "disable_triple_click" => "Desactivar triple clic",
        "tzdef" => "Zona horaria (POSIX)",
        "led_heartbeat_disabled" => "Apagar el LED de latido",
        "buzzer_mode" => "Modo del zumbador",
        "button_gpio" => "Pin del botón",
        "buzzer_gpio" => "Pin del zumbador",
        "serial_enabled" => "Consola serie",
        "position_broadcast_secs" => "Enviar posición cada (s)",
        "position_broadcast_smart_enabled" => "Posición inteligente",
        "fixed_position" => "Posición fija",
        "gps_enabled" => "GPS activado",
        "gps_update_interval" => "Actualizar GPS cada (s)",
        "gps_attempt_time" => "Tiempo de búsqueda del GPS (s)",
        "position_flags" => "Datos incluidos en la posición",
        "rx_gpio" => "Pin RX",
        "tx_gpio" => "Pin TX",
        "broadcast_smart_minimum_distance" => "Distancia mínima (m)",
        "broadcast_smart_minimum_interval_secs" => "Intervalo mínimo (s)",
        "gps_en_gpio" => "Pin de activación del GPS",
        "gps_mode" => "Modo del GPS",
        "is_power_saving" => "Ahorro de energía",
        "on_battery_shutdown_after_secs" => "Apagar con batería tras (s)",
        "adc_multiplier_override" => "Multiplicador ADC",
        "wait_bluetooth_secs" => "Esperar Bluetooth (s)",
        "sds_secs" => "Sueño profundo tras (s)",
        "ls_secs" => "Sueño ligero tras (s)",
        "min_wake_secs" => "Despierto como mínimo (s)",
        "device_battery_ina_address" => "Dirección INA de la batería",
        "powermon_enables" => "Monitor de energía",
        "wifi_enabled" => "WiFi activada",
        "wifi_ssid" => "Red WiFi (SSID)",
        "wifi_psk" => "Contraseña WiFi",
        "ntp_server" => "Servidor NTP",
        "eth_enabled" => "Ethernet activada",
        "address_mode" => "Direccionamiento",
        "ipv4_config" => "IPv4 estática",
        "ip" => "IP",
        "gateway" => "Puerta de enlace",
        "subnet" => "Máscara",
        "dns" => "DNS",
        "rsyslog_server" => "Servidor syslog",
        "enabled_protocols" => "Protocolos activados",
        "ipv6_enabled" => "IPv6",
        "screen_on_secs" => "Pantalla encendida (s)",
        "gps_format" => "Formato de coordenadas",
        "auto_screen_carousel_secs" => "Pasar pantallas cada (s)",
        "compass_north_top" => "Norte siempre arriba",
        "flip_screen" => "Girar la pantalla",
        "units" => "Unidades",
        "oled" => "Tipo de OLED",
        "displaymode" => "Modo de pantalla",
        "heading_bold" => "Títulos en negrita",
        "wake_on_tap_or_motion" => "Encender al tocar o mover",
        "compass_orientation" => "Orientación de la brújula",
        "use_12h_clock" => "Reloj de 12 horas",
        "use_long_node_name" => "Usar nombre largo",
        "use_preset" => "Usar preajuste",
        "modem_preset" => "Preajuste",
        "bandwidth" => "Ancho de banda (kHz)",
        "spread_factor" => "Factor de dispersión",
        "coding_rate" => "Tasa de codificación",
        "frequency_offset" => "Desvío de frecuencia (MHz)",
        "region" => "Región",
        "hop_limit" => "Límite de saltos",
        "tx_enabled" => "Emisión activada",
        "tx_power" => "Potencia (dBm, 0 = máxima legal)",
        "channel_num" => "Ranura de frecuencia",
        "override_duty_cycle" => "Saltarse el límite de ciclo de trabajo",
        "sx126x_rx_boosted_gain" => "Ganancia de recepción aumentada",
        "override_frequency" => "Frecuencia manual (MHz)",
        "pa_fan_disabled" => "Ventilador del amplificador apagado",
        "ignore_mqtt" => "Ignorar lo que llega por MQTT",
        "config_ok_to_mqtt" => "Permitir subir a MQTT",
        "fem_lna_mode" => "Modo FEM/LNA",
        "mode" => "Modo",
        "fixed_pin" => "PIN fijo",
        "is_managed" => "Gestionado",
        "serial_console_enabled" => "Consola serie",
        "debug_log_api_enabled" => "Registro de depuración por la API",
        "admin_channel_enabled" => "Canal de administración antiguo",
        "enabled" => "Activado",
        "address" => "Servidor",
        "username" => "Usuario",
        "password" => "Contraseña",
        "encryption_enabled" => "Cifrado",
        "json_enabled" => "JSON",
        "tls_enabled" => "TLS",
        "root" => "Tema raíz",
        "proxy_to_client_enabled" => "Usar la conexión del cliente",
        "map_reporting_enabled" => "Aparecer en el mapa público",
        "map_report_settings" => "Ajustes del mapa público",
        "publish_interval_secs" => "Publicar cada (s)",
        "position_precision" => "Precisión de la posición (bits)",
        "should_report_location" => "Informar de la ubicación",
        "device_update_interval" => "Métricas del aparato cada (s)",
        "environment_update_interval" => "Entorno cada (s)",
        "environment_measurement_enabled" => "Medir el entorno",
        "environment_screen_enabled" => "Entorno en pantalla",
        "environment_display_fahrenheit" => "Grados Fahrenheit",
        "air_quality_enabled" => "Calidad del aire",
        "air_quality_interval" => "Calidad del aire cada (s)",
        "power_measurement_enabled" => "Medir energía",
        "power_update_interval" => "Energía cada (s)",
        "power_screen_enabled" => "Energía en pantalla",
        "health_measurement_enabled" => "Medir salud",
        "health_update_interval" => "Salud cada (s)",
        "health_screen_enabled" => "Salud en pantalla",
        "device_telemetry_enabled" => "Enviar métricas del aparato",
        "update_interval" => "Actualizar cada (s)",
        "transmit_over_lora" => "Enviar por LoRa",
        "sender" => "Enviar cada (s)",
        "save" => "Guardar en fichero",
        "heartbeat" => "Latido",
        "records" => "Mensajes guardados",
        "history_return_max" => "Máximo de mensajes devueltos",
        "history_return_window" => "Ventana de tiempo (min)",
        "is_server" => "Hacer de servidor",
        "echo" => "Eco",
        "baud" => "Velocidad",
        "timeout" => "Tiempo de espera",
        "output_ms" => "Duración de la salida (ms)",
        "output" => "Pin de salida",
        "active" => "Activo en alto",
        "alert_message" => "Avisar con mensaje",
        "alert_bell" => "Avisar con campana",
        "use_pwm" => "Usar PWM",
        "nag_timeout" => "Repetir aviso durante (s)",
        "allow_input_source" => "Fuente de entrada",
        "send_bell" => "Enviar campana",
        "led_state" => "LED encendido",
        "current" => "Corriente",
        "red" => "Rojo",
        "green" => "Verde",
        "blue" => "Azul",
        "minimum_broadcast_secs" => "Enviar como mínimo cada (s)",
        "state_broadcast_secs" => "Estado cada (s)",
        "name" => "Nombre",
        "monitor_pin" => "Pin vigilado",
        "detection_trigger_type" => "Disparo",
        "use_pullup" => "Resistencia pull-up",
        "paxcounter_update_interval" => "Actualizar cada (s)",
        "wifi_threshold" => "Umbral WiFi (dBm)",
        "ble_threshold" => "Umbral Bluetooth (dBm)",
        "allow_undefined_pin_access" => "Permitir pines sin definir",
        "codec2_enabled" => "Codec2",
        "ptt_pin" => "Pin PTT",
        "bitrate" => "Velocidad",
        "rotary1_enabled" => "Rueda giratoria",
        "updown1_enabled" => "Teclas arriba/abajo",
        "send_bell_enabled" => "Enviar campana",
        "status_message" => "Mensaje de estado",
        _ => "",
    };
    if !s.is_empty() {
        return s.to_string();
    }
    let mut out = field.replace('_', " ");
    if let Some(f) = out.get_mut(0..1) {
        f.make_ascii_uppercase();
    }
    out
}

/// Lee el valor actual de un control.
type Reader = Box<dyn Fn() -> Value>;

fn build_fields(
    section: &str,
    obj: &Map<String, Value>,
    path: Vec<String>,
    group: &adw::PreferencesGroup,
    nested: &mut Vec<adw::PreferencesGroup>,
    readers: &mut Vec<(Vec<String>, Reader)>,
) {
    for (k, v) in obj {
        if HIDDEN.contains(&k.as_str()) {
            continue;
        }
        let mut p = path.clone();
        p.push(k.clone());
        let title = label(k);
        match v {
            Value::Bool(b) => {
                let r = adw::SwitchRow::builder().title(&title).active(*b).build();
                group.add(&r);
                readers.push((p, Box::new(move || Value::Bool(r.is_active()))));
            }
            Value::Number(n) => {
                if let Some(opts) = enum_options(section, k) {
                    let names: Vec<&str> = opts.iter().map(|o| o.1.as_str()).collect();
                    let model = gtk::StringList::new(&names);
                    let cur = n.as_i64().unwrap_or(0) as i32;
                    let r = adw::ComboRow::builder().title(&title).model(&model).build();
                    if let Some(i) = opts.iter().position(|o| o.0 == cur) {
                        r.set_selected(i as u32);
                    }
                    group.add(&r);
                    readers.push((p, Box::new(move || Value::from(opts.get(r.selected() as usize).map(|o| o.0).unwrap_or(0)))));
                } else if n.is_f64() {
                    let adj = gtk::Adjustment::new(n.as_f64().unwrap_or(0.0), -1e7, 1e7, 0.1, 1.0, 0.0);
                    let r = adw::SpinRow::builder().title(&title).adjustment(&adj).digits(2).build();
                    group.add(&r);
                    readers.push((p, Box::new(move || Value::from(r.value()))));
                } else {
                    let signed = n.is_i64() && !n.is_u64();
                    let cur = n.as_f64().unwrap_or(0.0);
                    let (lo, hi) = if signed { (i32::MIN as f64, i32::MAX as f64) } else { (0.0, u32::MAX as f64) };
                    let adj = gtk::Adjustment::new(cur, lo, hi, 1.0, 10.0, 0.0);
                    let r = adw::SpinRow::builder().title(&title).adjustment(&adj).digits(0).build();
                    group.add(&r);
                    readers.push((
                        p,
                        Box::new(move || if signed { Value::from(r.value() as i64) } else { Value::from(r.value().max(0.0) as u64) }),
                    ));
                }
            }
            Value::String(s) => {
                let secret = k.contains("psk") || k.contains("password");
                if secret {
                    let r = adw::PasswordEntryRow::builder().title(&title).text(s).build();
                    group.add(&r);
                    readers.push((p, Box::new(move || Value::from(r.text().to_string()))));
                } else {
                    let r = adw::EntryRow::builder().title(&title).text(s).build();
                    group.add(&r);
                    readers.push((p, Box::new(move || Value::from(r.text().to_string()))));
                }
            }
            Value::Object(o) => {
                let g = adw::PreferencesGroup::builder().title(&title).build();
                build_fields(section, o, p, &g, nested, readers);
                nested.push(g);
            }
            _ => {} // listas y valores vacíos: se conservan tal cual
        }
    }
}

fn set_path(v: &mut Value, path: &[String], new: Value) {
    let mut cur = v;
    for k in &path[..path.len() - 1] {
        let Some(next) = cur.get_mut(k) else { return };
        cur = next;
    }
    if let Some(last) = path.last() {
        if let Some(o) = cur.as_object_mut() {
            o.insert(last.clone(), new);
        }
    }
}

impl App {
    /// Página de edición de una sección de radio o de un módulo.
    pub(super) fn config_page(self: &Rc<Self>, kind: Kind, key: &str, title: &str) -> adw::NavigationPage {
        let page = adw::PreferencesPage::new();
        let section = {
            let st = self.st.borrow();
            match kind {
                Kind::Radio => st.config.get(key).and_then(section_value),
                Kind::Module => st.modules.get(key).and_then(section_value),
            }
        };
        let save = gtk::Button::builder().label("Guardar").sensitive(false).build();
        save.add_css_class("suggested-action");

        match section {
            None => {
                let s = adw::StatusPage::builder()
                    .icon_name("network-offline-symbolic")
                    .title("Sin datos")
                    .description("Conéctate al nodo para leer esta sección.")
                    .build();
                let g = adw::PreferencesGroup::new();
                g.add(&s);
                page.add(&g);
            }
            Some((k, value)) => {
                let warn = match key {
                    "lora" => Some("Cambiar la región, el preajuste o la ranura te separa de los nodos que usan otros valores. El nodo se reinicia al guardar."),
                    "bluetooth" => Some("Si cambias el modo o el PIN tendrás que volver a emparejar el nodo."),
                    "network" => Some("Con la WiFi activada, algunos aparatos apagan el Bluetooth."),
                    "security" => Some("Desactivar la consola serie o la API puede dejarte sin acceso por USB."),
                    "mqtt" => Some("El nodo necesita WiFi o «Usar la conexión del cliente» para llegar al servidor."),
                    _ => None,
                };
                let g = adw::PreferencesGroup::builder().description(warn.unwrap_or("")).build();
                let mut readers = vec![];
                let mut nested = vec![];
                if let Some(o) = value.as_object() {
                    build_fields(&k, o, vec![], &g, &mut nested, &mut readers);
                }
                page.add(&g);
                for n in &nested {
                    page.add(n);
                }
                save.set_sensitive(true);
                let a = self.clone();
                let key = key.to_string();
                let readers = Rc::new(readers);
                save.connect_clicked(move |_| {
                    let mut v = value.clone();
                    for (p, r) in readers.iter() {
                        set_path(&mut v, p, r());
                    }
                    let wrapped = serde_json::json!({ "payload_variant": { k.clone(): v } });
                    let msg = match kind {
                        Kind::Radio => serde_json::from_value::<protobufs::Config>(wrapped).map(|c| {
                            a.st.borrow_mut().config.insert(key.clone(), c.clone());
                            protobufs::admin_message::PayloadVariant::SetConfig(c)
                        }),
                        Kind::Module => serde_json::from_value::<protobufs::ModuleConfig>(wrapped).map(|c| {
                            a.st.borrow_mut().modules.insert(key.clone(), c.clone());
                            protobufs::admin_message::PayloadVariant::SetModuleConfig(c)
                        }),
                    };
                    match msg {
                        Ok(m) => a.admin(vec![m]),
                        Err(e) => a.toast(&format!("No se pudo preparar la configuración: {e}")),
                    }
                });
            }
        }

        let header = adw::HeaderBar::new();
        header.pack_end(&save);
        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&header);
        tv.set_content(Some(&page));
        adw::NavigationPage::builder().title(title).child(&tv).build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protobufs::config::PayloadVariant as C;
    use protobufs::module_config::PayloadVariant as M;

    /// JSON y vuelta tiene que dar exactamente lo mismo, o al guardar se perderían campos.
    #[test]
    fn ida_y_vuelta() {
        let lora = cfg::LoRaConfig {
            use_preset: true,
            region: cfg::lo_ra_config::RegionCode::Eu868 as i32,
            modem_preset: 3,
            hop_limit: 5,
            tx_power: 27,
            frequency_offset: 0.5,
            ignore_incoming: vec![1, 2, 3],
            ..Default::default()
        };
        let sec = cfg::SecurityConfig { public_key: vec![9; 32], private_key: vec![7; 32], admin_key: vec![vec![5; 32]], ..Default::default() };
        let net = cfg::NetworkConfig {
            wifi_ssid: "casa".into(),
            ipv4_config: Some(cfg::network_config::IpV4Config { ip: 1, gateway: 2, subnet: 3, dns: 4 }),
            ..Default::default()
        };
        let cfgs = vec![
            C::Device(cfg::DeviceConfig { role: 2, node_info_broadcast_secs: 900, tzdef: "CET".into(), ..Default::default() }),
            C::Position(cfg::PositionConfig::default()),
            C::Power(cfg::PowerConfig { adc_multiplier_override: 1.5, ..Default::default() }),
            C::Network(net),
            C::Display(cfg::DisplayConfig::default()),
            C::Lora(lora),
            C::Bluetooth(cfg::BluetoothConfig { fixed_pin: 123456, ..Default::default() }),
            C::Security(sec),
        ];
        for c in cfgs {
            let c = protobufs::Config { payload_variant: Some(c) };
            let (k, v) = section_value(&c).unwrap();
            let back: protobufs::Config = serde_json::from_value(serde_json::json!({ "payload_variant": { k: v } })).unwrap();
            assert_eq!(c, back);
        }
        let mods = vec![
            M::Mqtt(mcfg::MqttConfig { address: "x".into(), map_report_settings: Some(Default::default()), ..Default::default() }),
            M::Telemetry(mcfg::TelemetryConfig::default()),
            M::Serial(mcfg::SerialConfig::default()),
            M::CannedMessage(mcfg::CannedMessageConfig::default()),
            M::ExternalNotification(mcfg::ExternalNotificationConfig::default()),
            M::StoreForward(mcfg::StoreForwardConfig::default()),
        ];
        for m in mods {
            let m = protobufs::ModuleConfig { payload_variant: Some(m) };
            let (k, v) = section_value(&m).unwrap();
            let back: protobufs::ModuleConfig = serde_json::from_value(serde_json::json!({ "payload_variant": { k: v } })).unwrap();
            assert_eq!(m, back);
        }
    }

    /// Cambiar un campo con set_path no toca los demás.
    #[test]
    fn cambiar_un_campo() {
        let c = protobufs::Config {
            payload_variant: Some(C::Lora(cfg::LoRaConfig { hop_limit: 3, region: 3, ..Default::default() })),
        };
        let (k, mut v) = section_value(&c).unwrap();
        set_path(&mut v, &["hop_limit".to_string()], Value::from(5u64));
        let back: protobufs::Config = serde_json::from_value(serde_json::json!({ "payload_variant": { k: v } })).unwrap();
        match back.payload_variant {
            Some(C::Lora(l)) => {
                assert_eq!(l.hop_limit, 5);
                assert_eq!(l.region, 3);
            }
            _ => panic!(),
        }
    }
}
