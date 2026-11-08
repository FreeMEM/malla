//! Pestaña Ajustes: tu nodo, canales, radio, módulos, administración y la app.

use std::io::Read;

use base64::Engine;
use meshtastic::Message as _;

use super::config::{Kind, MODULES, RADIO};
use super::*;
use protobufs::admin_message::PayloadVariant as P;

const URL_PREFIX: &str = "https://meshtastic.org/e/#";

fn nav_row(title: &str, sub: &str) -> adw::ActionRow {
    let r = adw::ActionRow::builder().title(title).subtitle(sub).activatable(true).use_markup(false).build();
    r.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    r
}

fn page_with(title: &str, content: &impl IsA<gtk::Widget>, end: Option<&gtk::Button>) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    if let Some(b) = end {
        header.pack_end(b);
    }
    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&header);
    tv.set_content(Some(content));
    adw::NavigationPage::builder().title(title).child(&tv).build()
}

fn random_key() -> Vec<u8> {
    let mut k = vec![0u8; 32];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut k);
    }
    k
}

fn psk_desc(psk: &[u8]) -> &'static str {
    match psk.len() {
        0 => "sin cifrado",
        1 => "clave por defecto (pública)",
        16 => "AES-128",
        32 => "AES-256",
        _ => "clave",
    }
}

/// Código QR como textura.
fn qr_texture(text: &str) -> Option<gdk::MemoryTexture> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let n = code.width();
    let colors = code.to_colors();
    let (scale, border) = (6usize, 4usize);
    let size = (n + border * 2) * scale;
    let mut px = vec![255u8; size * size * 4];
    for y in 0..n {
        for x in 0..n {
            if colors[y * n + x] == qrcode::Color::Dark {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let (xx, yy) = ((x + border) * scale + dx, (y + border) * scale + dy);
                        let i = (yy * size + xx) * 4;
                        px[i..i + 3].copy_from_slice(&[0, 0, 0]);
                    }
                }
            }
        }
    }
    Some(gdk::MemoryTexture::new(size as i32, size as i32, gdk::MemoryFormat::R8g8b8a8, &glib::Bytes::from_owned(px), size * 4))
}

impl App {
    pub(super) fn open_settings_section(self: &Rc<Self>, key: &str) {
        self.w.stack.set_visible_child_name("ajustes");
        self.w.settings_nav.pop_to_tag("raiz");
        let page = match key {
            "usuario" => self.user_page(),
            "canales" => self.channels_page(),
            "posicion_fija" => self.fixed_position_page(),
            "info" => self.info_page(),
            "app" => self.app_page(),
            k => {
                if let Some((_, t, _)) = RADIO.iter().find(|r| r.0 == k) {
                    self.config_page(Kind::Radio, k, t)
                } else if let Some((_, t, _)) = MODULES.iter().find(|r| r.0 == k) {
                    self.config_page(Kind::Module, k, t)
                } else {
                    return;
                }
            }
        };
        self.w.settings_nav.push(&page);
    }

    pub(super) fn rebuild_settings(self: &Rc<Self>) {
        let root = &self.w.settings_root;
        while let Some(c) = root.first_child() {
            root.remove(&c);
        }
        let page = adw::PreferencesPage::new();
        page.set_vexpand(true);
        let st = self.st.borrow();

        let g = adw::PreferencesGroup::builder().title("Tu nodo").build();
        let conn = adw::ActionRow::builder()
            .title(if st.connected { "Conectado" } else if st.connecting { "Conectando…" } else { "Desconectado" })
            .subtitle(st.target.as_ref().map(|t| t.label()).unwrap_or_else(|| "Pulsa Conectar arriba a la izquierda".into()))
            .use_markup(false)
            .build();
        let b = gtk::Button::builder().label(if st.connected { "Desconectar" } else { "Conectar…" }).valign(gtk::Align::Center).build();
        if st.connected {
            b.add_css_class("destructive-action");
        }
        let a = self.clone();
        let connected = st.connected;
        b.connect_clicked(move |_| if connected { a.send_cmd(Command::Disconnect) } else { a.open_connect_dialog() });
        conn.add_suffix(&b);
        g.add(&conn);

        let me = st.nodes.get(&st.my);
        let user_sub = me.map(|n| format!("{} ({})", n.display_name(), n.short_name)).unwrap_or_else(|| "Nombre largo y corto".into());
        let chans = st.channels_full.values().filter(|c| c.role != 0).count();
        let rows = [
            ("usuario", "Usuario", user_sub),
            ("canales", "Canales", format!("{chans} activos · compartir por QR o enlace")),
            ("posicion_fija", "Posición fija", "Fijar la posición sin GPS".to_string()),
            (
                "info",
                "Información del aparato",
                st.metadata.as_ref().map(|m| format!("Firmware {}", m.firmware_version)).unwrap_or_else(|| "Firmware y capacidades".into()),
            ),
        ];
        for (k, t, s) in rows {
            let r = nav_row(t, &s);
            let a = self.clone();
            r.connect_activated(move |_| a.open_settings_section(k));
            g.add(&r);
        }
        page.add(&g);

        let g = adw::PreferencesGroup::builder().title("Radio").build();
        for (k, t, s) in RADIO {
            let r = nav_row(t, s);
            let a = self.clone();
            r.connect_activated(move |_| a.open_settings_section(k));
            g.add(&r);
        }
        page.add(&g);

        let g = adw::PreferencesGroup::builder().title("Módulos").build();
        for (k, t, s) in MODULES {
            if st.connected && !st.modules.is_empty() && !st.modules.contains_key(k) {
                continue; // el firmware no lo tiene
            }
            let on = st.modules.get(k).and_then(|m| serde_json::to_value(m).ok()).and_then(|v| {
                v.get("payload_variant")?.as_object()?.values().next()?.get("enabled")?.as_bool()
            });
            let sub = match on {
                Some(true) => format!("Activado · {s}"),
                Some(false) => format!("Desactivado · {s}"),
                None => s.to_string(),
            };
            let r = nav_row(t, &sub);
            let a = self.clone();
            r.connect_activated(move |_| a.open_settings_section(k));
            g.add(&r);
        }
        page.add(&g);

        let g = adw::PreferencesGroup::builder()
            .title("Administración")
            .description("Acciones sobre el nodo conectado. Piden confirmación.")
            .build();
        let can_shutdown = st.metadata.as_ref().map(|m| m.can_shutdown).unwrap_or(true);
        let actions: Vec<(&str, &str, &str, P)> = vec![
            ("Reiniciar el nodo", "Se reinicia en 5 segundos y Malla vuelve a conectar sola.", "Reiniciar", P::RebootSeconds(5)),
            ("Apagar el nodo", "Se apaga en 5 segundos. Para encenderlo hay que pulsar su botón o desenchufarlo.", "Apagar", P::ShutdownSeconds(5)),
            ("Borrar la lista de nodos", "El nodo olvida todos los nodos conocidos. Se irán llenando de nuevo.", "Borrar", P::NodedbReset(true)),
            ("Restablecer la configuración", "Vuelve a los valores de fábrica la configuración, pero guarda las claves.", "Restablecer", P::FactoryResetConfig(1)),
            ("Restablecer de fábrica", "Borra todo, incluidas las claves de cifrado del nodo. No se puede deshacer.", "Borrar todo", P::FactoryResetDevice(1)),
        ];
        for (t, body, verb, payload) in actions {
            if t.starts_with("Apagar") && !can_shutdown {
                continue;
            }
            let r = adw::ActionRow::builder().title(t).activatable(true).build();
            if verb != "Reiniciar" {
                r.add_css_class("error");
            }
            let a = self.clone();
            let payload = Rc::new(payload);
            r.connect_activated(move |_| {
                let a2 = a.clone();
                let p = payload.clone();
                a.confirm(&format!("¿{t}?"), body, verb, move || a2.admin(vec![(*p).clone()]));
            });
            g.add(&r);
        }
        page.add(&g);

        let g = adw::PreferencesGroup::builder().title("Aplicación").build();
        let r = nav_row("Preferencias de Malla", "Avisos, arranque, tema, respuestas rápidas y datos");
        let a = self.clone();
        r.connect_activated(move |_| a.open_settings_section("app"));
        g.add(&r);
        page.add(&g);

        drop(st);
        root.append(&page);
    }

    fn user_page(self: &Rc<Self>) -> adw::NavigationPage {
        let page = adw::PreferencesPage::new();
        let user = self.st.borrow().my_user.clone();
        let save = gtk::Button::builder().label("Guardar").sensitive(user.is_some()).build();
        save.add_css_class("suggested-action");
        let g = adw::PreferencesGroup::builder()
            .description("Así te ven los demás. El nombre corto sale en los mapas y en las pantallas pequeñas.")
            .build();
        let long = adw::EntryRow::builder().title("Nombre largo (hasta 39 letras)").build();
        let short = adw::EntryRow::builder().title("Nombre corto (hasta 4 letras)").build();
        let lic = adw::SwitchRow::builder()
            .title("Radioaficionado con licencia")
            .subtitle("Usa tu indicativo como nombre. Desactiva el cifrado, como exige la ley.")
            .build();
        if let Some(u) = &user {
            long.set_text(&u.long_name);
            short.set_text(&u.short_name);
            lic.set_active(u.is_licensed);
        }
        g.add(&long);
        g.add(&short);
        g.add(&lic);
        page.add(&g);
        let a = self.clone();
        save.connect_clicked(move |_| {
            let Some(mut u) = a.st.borrow().my_user.clone() else { return };
            let l: String = long.text().trim().chars().take(39).collect();
            let s: String = short.text().trim().chars().take(4).collect();
            if l.is_empty() || s.is_empty() {
                a.toast("Los dos nombres son obligatorios");
                return;
            }
            u.long_name = l;
            u.short_name = s;
            u.is_licensed = lic.is_active();
            a.st.borrow_mut().my_user = Some(u.clone());
            a.admin(vec![P::SetOwner(u)]);
        });
        page_with("Usuario", &page, Some(&save))
    }

    fn channel_url(&self) -> Option<String> {
        let st = self.st.borrow();
        let settings: Vec<protobufs::ChannelSettings> =
            st.channels_full.values().filter(|c| c.role != 0).filter_map(|c| c.settings.clone()).collect();
        if settings.is_empty() {
            return None;
        }
        let lora = st.config.get("lora").and_then(|c| match &c.payload_variant {
            Some(protobufs::config::PayloadVariant::Lora(l)) => Some(l.clone()),
            _ => None,
        });
        let set = protobufs::ChannelSet { settings, lora_config: lora };
        Some(format!("{URL_PREFIX}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(set.encode_to_vec())))
    }

    fn channels_page(self: &Rc<Self>) -> adw::NavigationPage {
        let page = adw::PreferencesPage::new();
        let st = self.st.borrow();
        let g = adw::PreferencesGroup::builder()
            .title("Canales del nodo")
            .description("El primario lleva la posición y la telemetría. Hasta 7 secundarios para grupos privados.")
            .build();
        let mut free = None;
        for i in 0..8u32 {
            match st.channels_full.get(&i) {
                Some(c) if c.role != 0 => {
                    let s = c.settings.clone().unwrap_or_default();
                    let name = if s.name.is_empty() { st.channel_name(i) } else { s.name.clone() };
                    let mut sub = vec![if c.role == 1 { "primario".to_string() } else { "secundario".to_string() }, psk_desc(&s.psk).to_string()];
                    if s.uplink_enabled {
                        sub.push("sube a MQTT".into());
                    }
                    if s.downlink_enabled {
                        sub.push("baja de MQTT".into());
                    }
                    let r = nav_row(&format!("{i} · {name}"), &sub.join(" · "));
                    let a = self.clone();
                    r.connect_activated(move |_| {
                        let p = a.channel_edit_page(i);
                        a.w.settings_nav.push(&p);
                    });
                    g.add(&r);
                }
                _ => {
                    if free.is_none() && i > 0 {
                        free = Some(i);
                    }
                }
            }
        }
        if st.channels_full.is_empty() {
            g.add(&adw::ActionRow::builder().title("Conéctate al nodo para ver sus canales").build());
        }
        if let Some(i) = free.filter(|_| st.connected) {
            let add = gtk::Button::builder().label("Añadir canal").valign(gtk::Align::Center).build();
            let a = self.clone();
            add.connect_clicked(move |_| {
                let p = a.channel_edit_page(i);
                a.w.settings_nav.push(&p);
            });
            g.set_header_suffix(Some(&add));
        }
        page.add(&g);
        drop(st);

        // Compartir.
        if let Some(url) = self.channel_url() {
            let g = adw::PreferencesGroup::builder()
                .title("Compartir")
                .description("Escanéalo con la app del móvil para unirte a estos canales. Cualquiera con el código puede leerlos.")
                .build();
            if let Some(t) = qr_texture(&url) {
                let pic = gtk::Picture::for_paintable(&t);
                pic.set_size_request(260, 260);
                pic.set_can_shrink(true);
                pic.set_halign(gtk::Align::Center);
                pic.set_margin_bottom(12);
                g.add(&pic);
            }
            let r = adw::ActionRow::builder().title("Enlace").subtitle(&url).subtitle_lines(3).use_markup(false).build();
            r.add_css_class("property");
            let copy = gtk::Button::builder().icon_name("edit-copy-symbolic").tooltip_text("Copiar").valign(gtk::Align::Center).build();
            copy.add_css_class("flat");
            let a = self.clone();
            let u = url.clone();
            copy.connect_clicked(move |b| {
                b.clipboard().set_text(&u);
                a.toast("Enlace copiado");
            });
            r.add_suffix(&copy);
            g.add(&r);
            page.add(&g);
        }

        // Importar.
        let g = adw::PreferencesGroup::builder()
            .title("Importar")
            .description("Pega un enlace meshtastic.org/e/#… para sustituir los canales del nodo.")
            .build();
        let entry = adw::EntryRow::builder().title("Enlace de canales").build();
        let go = gtk::Button::builder().label("Importar…").valign(gtk::Align::Center).build();
        entry.add_suffix(&go);
        g.add(&entry);
        page.add(&g);
        let a = self.clone();
        go.connect_clicked(move |_| a.import_channels(entry.text().trim()));

        page_with("Canales", &page, None)
    }

    fn import_channels(self: &Rc<Self>, url: &str) {
        let b64 = url.split('#').nth(1).unwrap_or(url).split('?').next().unwrap_or("");
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(b64.trim_end_matches('='))
            .or_else(|_| base64::engine::general_purpose::STANDARD.decode(b64));
        let Ok(bytes) = bytes else {
            self.toast("El enlace no es válido");
            return;
        };
        let Ok(set) = protobufs::ChannelSet::decode(bytes.as_slice()) else {
            self.toast("El enlace no contiene canales de Meshtastic");
            return;
        };
        if set.settings.is_empty() {
            self.toast("El enlace no trae ningún canal");
            return;
        }
        let names: Vec<String> = set
            .settings
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{i}: {}", if s.name.is_empty() { "(por defecto)" } else { &s.name }))
            .collect();
        let body = format!(
            "Se sustituyen los canales del nodo por:\n{}\n{}",
            names.join("\n"),
            if set.lora_config.is_some() { "\nTambién se aplica la configuración LoRa del enlace y el nodo se reiniciará." } else { "" }
        );
        let a = self.clone();
        self.confirm("¿Importar canales?", &body, "Importar", move || {
            let mut list = vec![];
            for i in 0..8usize {
                let ch = match set.settings.get(i) {
                    Some(s) => protobufs::Channel { index: i as i32, settings: Some(s.clone()), role: if i == 0 { 1 } else { 2 } },
                    None => protobufs::Channel { index: i as i32, settings: None, role: 0 },
                };
                list.push(P::SetChannel(ch));
            }
            if let Some(l) = set.lora_config.clone() {
                list.push(P::SetConfig(protobufs::Config { payload_variant: Some(protobufs::config::PayloadVariant::Lora(l)) }));
            }
            a.admin(list);
        });
    }

    fn channel_edit_page(self: &Rc<Self>, index: u32) -> adw::NavigationPage {
        let page = adw::PreferencesPage::new();
        let existing = self.st.borrow().channels_full.get(&index).cloned().filter(|c| c.role != 0);
        let s = existing.as_ref().and_then(|c| c.settings.clone()).unwrap_or_default();
        let role = existing.as_ref().map(|c| c.role).unwrap_or(2);

        let g = adw::PreferencesGroup::builder()
            .description(if role == 1 {
                "Canal primario. Sin nombre usa el del preajuste LoRa (LongFast…). Cambiarlo te separa de la red pública."
            } else {
                "Canal secundario. Quien quiera hablar en él necesita el mismo nombre y la misma clave."
            })
            .build();
        let name = adw::EntryRow::builder().title("Nombre (hasta 11 letras)").text(&s.name).build();
        let key_model = gtk::StringList::new(&["Por defecto (pública)", "Aleatoria AES-256", "Sin cifrado", "Personalizada (base64)"]);
        let key = adw::ComboRow::builder().title("Clave").model(&key_model).build();
        let custom = adw::EntryRow::builder().title("Clave en base64").build();
        let cur_b64 = base64::engine::general_purpose::STANDARD.encode(&s.psk);
        custom.set_text(&cur_b64);
        key.set_selected(match s.psk.len() {
            1 if s.psk[0] == 1 => 0,
            0 => 2,
            _ => 3,
        });
        if existing.is_none() {
            key.set_selected(1);
        }
        let set_vis = {
            let custom = custom.clone();
            move |k: &adw::ComboRow| custom.set_visible(k.selected() == 3)
        };
        set_vis(&key);
        key.connect_selected_notify(set_vis);
        let up = adw::SwitchRow::builder().title("Subir a MQTT").active(s.uplink_enabled).build();
        let down = adw::SwitchRow::builder().title("Bajar de MQTT").active(s.downlink_enabled).build();
        let ms = s.module_settings.clone().unwrap_or_default();
        let prec = adw::SpinRow::builder()
            .title("Precisión de la posición")
            .subtitle("0 = no compartir; 32 = exacta; 13 ≈ 3 km")
            .adjustment(&gtk::Adjustment::new(ms.position_precision as f64, 0.0, 32.0, 1.0, 1.0, 0.0))
            .build();
        let muted = adw::SwitchRow::builder().title("Silenciado en el aparato").active(ms.is_muted).build();
        for w in [name.upcast_ref::<gtk::Widget>(), key.upcast_ref(), custom.upcast_ref(), up.upcast_ref(), down.upcast_ref(), prec.upcast_ref(), muted.upcast_ref()] {
            g.add(w);
        }
        page.add(&g);

        if role == 2 && existing.is_some() {
            let g = adw::PreferencesGroup::new();
            let del = gtk::Button::builder().label("Borrar este canal").halign(gtk::Align::Center).build();
            del.add_css_class("destructive-action");
            del.add_css_class("pill");
            let a = self.clone();
            del.connect_clicked(move |_| {
                let a2 = a.clone();
                a.confirm("¿Borrar el canal?", "Dejarás de recibir sus mensajes.", "Borrar", move || {
                    a2.admin(vec![P::SetChannel(protobufs::Channel { index: index as i32, settings: None, role: 0 })]);
                    a2.st.borrow_mut().channels_full.remove(&index);
                    a2.st.borrow_mut().channels.remove(&index);
                    a2.w.settings_nav.pop();
                    a2.rebuild_convs();
                });
            });
            g.add(&del);
            page.add(&g);
        }

        let save = gtk::Button::builder().label("Guardar").build();
        save.add_css_class("suggested-action");
        let a = self.clone();
        save.connect_clicked(move |_| {
            let mut s = s.clone();
            s.name = name.text().trim().chars().take(11).collect();
            s.psk = match key.selected() {
                0 => vec![1],
                1 => random_key(),
                2 => vec![],
                _ => match base64::engine::general_purpose::STANDARD.decode(custom.text().trim()) {
                    Ok(k) if [0, 1, 16, 32].contains(&k.len()) => k,
                    _ => {
                        a.toast("La clave debe ser base64 de 16 o 32 bytes");
                        return;
                    }
                },
            };
            if role == 2 && s.name.is_empty() {
                a.toast("Los canales secundarios necesitan nombre");
                return;
            }
            s.uplink_enabled = up.is_active();
            s.downlink_enabled = down.is_active();
            s.module_settings = Some(protobufs::ModuleSettings { position_precision: prec.value() as u32, is_muted: muted.is_active() });
            let ch = protobufs::Channel { index: index as i32, settings: Some(s.clone()), role };
            a.st.borrow_mut().channels_full.insert(index, ch.clone());
            a.st.borrow_mut().channels.insert(index, ChannelInfo { index, name: s.name.clone(), role });
            a.admin(vec![P::SetChannel(ch)]);
            a.rebuild_convs();
            a.w.settings_nav.pop();
        });
        page_with(if existing.is_some() { "Editar canal" } else { "Nuevo canal" }, &page, Some(&save))
    }

    fn fixed_position_page(self: &Rc<Self>) -> adw::NavigationPage {
        let page = adw::PreferencesPage::new();
        let (la, lo, alt) = {
            let st = self.st.borrow();
            let n = st.nodes.get(&st.my);
            (n.and_then(|n| n.lat).unwrap_or(0.0), n.and_then(|n| n.lon).unwrap_or(0.0), n.and_then(|n| n.alt).unwrap_or(0))
        };
        let g = adw::PreferencesGroup::builder()
            .description("Para nodos sin GPS o fijos en un tejado. También puedes hacer clic derecho en el mapa.")
            .build();
        let lat = adw::SpinRow::builder().title("Latitud").digits(6).adjustment(&gtk::Adjustment::new(la, -90.0, 90.0, 0.0001, 0.01, 0.0)).build();
        let lon = adw::SpinRow::builder().title("Longitud").digits(6).adjustment(&gtk::Adjustment::new(lo, -180.0, 180.0, 0.0001, 0.01, 0.0)).build();
        let al = adw::SpinRow::builder().title("Altitud (m)").adjustment(&gtk::Adjustment::new(alt as f64, -500.0, 9000.0, 1.0, 10.0, 0.0)).build();
        g.add(&lat);
        g.add(&lon);
        g.add(&al);
        page.add(&g);
        let g = adw::PreferencesGroup::new();
        let remove = gtk::Button::builder().label("Quitar la posición fija").halign(gtk::Align::Center).build();
        remove.add_css_class("pill");
        let a = self.clone();
        remove.connect_clicked(move |_| a.admin(vec![P::RemoveFixedPosition(true)]));
        g.add(&remove);
        page.add(&g);
        let save = gtk::Button::builder().label("Fijar").build();
        save.add_css_class("suggested-action");
        let a = self.clone();
        save.connect_clicked(move |_| a.set_fixed_position(lat.value(), lon.value(), al.value() as i32));
        page_with("Posición fija", &page, Some(&save))
    }

    fn info_page(self: &Rc<Self>) -> adw::NavigationPage {
        let page = adw::PreferencesPage::new();
        let st = self.st.borrow();
        let g = adw::PreferencesGroup::new();
        let row = |t: &str, v: String| {
            let r = adw::ActionRow::builder().title(t).subtitle(v).use_markup(false).build();
            r.add_css_class("property");
            g.add(&r);
        };
        row("Identificador", Node::id_str(st.my));
        row("Conexión", st.target.as_ref().map(|t| t.label()).unwrap_or_else(|| "ninguna".into()));
        row("Nodos conocidos", st.nodes.len().to_string());
        if let Some(m) = &st.metadata {
            row("Firmware", m.firmware_version.clone());
            row("Modelo", hw_name(m.hw_model));
            row("Función", role_name(m.role));
            let yes = |b: bool| if b { "sí" } else { "no" };
            row("WiFi", yes(m.has_wifi).into());
            row("Bluetooth", yes(m.has_bluetooth).into());
            row("Ethernet", yes(m.has_ethernet).into());
            row("Cifrado de clave pública", yes(m.has_pkc).into());
            row("Puede apagarse", yes(m.can_shutdown).into());
            row("Versión del estado", m.device_state_version.to_string());
        } else {
            row("Firmware", "conéctate para leerlo".into());
        }
        page.add(&g);
        drop(st);
        page_with("Información del aparato", &page, None)
    }

    fn app_page(self: &Rc<Self>) -> adw::NavigationPage {
        let page = adw::PreferencesPage::new();
        let prefs = self.st.borrow().saved.prefs.clone();

        let g = adw::PreferencesGroup::builder().title("Avisos").build();
        let notify = adw::SwitchRow::builder().title("Avisar al llegar mensajes").active(prefs.notify).build();
        let notify_ch = adw::SwitchRow::builder()
            .title("Avisar también de los canales")
            .subtitle("Si lo apagas, solo avisan los mensajes directos")
            .active(prefs.notify_channels)
            .build();
        g.add(&notify);
        g.add(&notify_ch);
        page.add(&g);

        let g = adw::PreferencesGroup::builder().title("Arranque y aspecto").build();
        let auto = adw::SwitchRow::builder().title("Abrir Malla al iniciar la sesión").active(store::autostart_path().exists()).build();
        let hidden = adw::SwitchRow::builder()
            .title("Arrancar escondida en la bandeja")
            .active(prefs.start_hidden)
            .build();
        let theme_model = gtk::StringList::new(&["Como el sistema", "Claro", "Oscuro"]);
        let theme = adw::ComboRow::builder().title("Tema").model(&theme_model).build();
        theme.set_selected(match prefs.theme.as_str() {
            "claro" => 1,
            "oscuro" => 2,
            _ => 0,
        });
        g.add(&auto);
        g.add(&hidden);
        g.add(&theme);
        page.add(&g);

        let a = self.clone();
        notify.connect_active_notify(move |r| {
            a.st.borrow_mut().saved.prefs.notify = r.is_active();
            a.save();
        });
        let a = self.clone();
        notify_ch.connect_active_notify(move |r| {
            a.st.borrow_mut().saved.prefs.notify_channels = r.is_active();
            a.save();
        });
        let a = self.clone();
        auto.connect_active_notify(move |r| {
            if let Err(e) = store::set_autostart(r.is_active()) {
                a.toast(&format!("No se pudo cambiar el arranque: {e}"));
            }
        });
        let a = self.clone();
        hidden.connect_active_notify(move |r| {
            a.st.borrow_mut().saved.prefs.start_hidden = r.is_active();
            a.save();
        });
        let a = self.clone();
        theme.connect_selected_notify(move |r| {
            let t = ["", "claro", "oscuro"][r.selected() as usize % 3].to_string();
            apply_theme(&t);
            a.st.borrow_mut().saved.prefs.theme = t;
            a.save();
        });

        // Respuestas rápidas.
        let g = adw::PreferencesGroup::builder()
            .title("Respuestas rápidas")
            .description("Salen encima del cuadro de texto. Un clic las escribe.")
            .build();
        let rows: Rc<RefCell<Vec<adw::EntryRow>>> = Rc::new(RefCell::new(vec![]));
        let add_row = {
            let g = g.clone();
            let rows = rows.clone();
            move |text: &str| {
                let r = adw::EntryRow::builder().title("Frase").text(text).build();
                let del = gtk::Button::builder().icon_name("user-trash-symbolic").valign(gtk::Align::Center).build();
                del.add_css_class("flat");
                let gg = g.clone();
                let rr = r.clone();
                let rows2 = rows.clone();
                del.connect_clicked(move |_| {
                    gg.remove(&rr);
                    rows2.borrow_mut().retain(|x| x != &rr);
                });
                r.add_suffix(&del);
                g.add(&r);
                rows.borrow_mut().push(r);
            }
        };
        for q in &prefs.quick {
            add_row(q);
        }
        let bx = gtk::Box::builder().spacing(6).build();
        let addb = gtk::Button::builder().icon_name("list-add-symbolic").tooltip_text("Añadir").build();
        addb.add_css_class("flat");
        let saveq = gtk::Button::with_label("Guardar");
        bx.append(&addb);
        bx.append(&saveq);
        g.set_header_suffix(Some(&bx));
        let add2 = add_row.clone();
        addb.connect_clicked(move |_| add2(""));
        let a = self.clone();
        let rows2 = rows.clone();
        saveq.connect_clicked(move |_| {
            let v: Vec<String> = rows2.borrow().iter().map(|r| r.text().trim().to_string()).filter(|s| !s.is_empty()).collect();
            a.st.borrow_mut().saved.prefs.quick = v;
            a.save();
            a.rebuild_quick();
            a.toast("Respuestas rápidas guardadas");
        });
        page.add(&g);

        // Datos.
        let g = adw::PreferencesGroup::builder()
            .title("Datos guardados")
            .description(format!("En {}", glib::user_data_dir().join("malla").display()))
            .build();
        let (nm, nt, nw) = {
            let st = self.st.borrow();
            (st.saved.messages.len(), st.saved.telemetry.values().map(|v| v.len()).sum::<usize>(), st.saved.waypoints.len())
        };
        let clear_msgs = adw::ActionRow::builder().title("Borrar el historial de mensajes").subtitle(format!("{nm} mensajes")).activatable(true).build();
        let clear_tel = adw::ActionRow::builder()
            .title("Borrar telemetría, rastros y traceroutes")
            .subtitle(format!("{nt} medidas"))
            .activatable(true)
            .build();
        let clear_wp = adw::ActionRow::builder().title("Borrar los puntos del mapa").subtitle(format!("{nw} puntos")).activatable(true).build();
        for r in [&clear_msgs, &clear_tel, &clear_wp] {
            r.add_css_class("error");
            g.add(r);
        }
        let a = self.clone();
        clear_msgs.connect_activated(move |_| {
            let a2 = a.clone();
            a.confirm("¿Borrar los mensajes?", "Se borra el historial de este equipo. La radio no guarda copia.", "Borrar", move || {
                a2.st.borrow_mut().saved.messages.clear();
                a2.save();
                a2.rebuild_convs();
                a2.rebuild_messages();
            });
        });
        let a = self.clone();
        clear_tel.connect_activated(move |_| {
            let a2 = a.clone();
            a.confirm("¿Borrar la telemetría?", "Se borran gráficas, rastros y traceroutes guardados en este equipo.", "Borrar", move || {
                let mut st = a2.st.borrow_mut();
                st.saved.telemetry.clear();
                st.saved.tracks.clear();
                st.saved.traces.clear();
                drop(st);
                a2.save();
            });
        });
        let a = self.clone();
        clear_wp.connect_activated(move |_| {
            let a2 = a.clone();
            a.confirm("¿Borrar los puntos?", "Solo se borran en este equipo; el resto de la malla los conserva.", "Borrar", move || {
                a2.st.borrow_mut().saved.waypoints.clear();
                a2.save();
                a2.rebuild_waypoints();
            });
        });
        page.add(&g);

        page_with("Preferencias de Malla", &page, None)
    }
}
