//! Ventana de conexión.

use super::*;

impl App {

    pub(super) fn open_connect_dialog(self: &Rc<Self>) {
        let page = adw::PreferencesPage::new();
        let st = self.st.borrow();

        if st.connected {
            let g = adw::PreferencesGroup::builder().title("Conexión actual").build();
            let r = adw::ActionRow::builder()
                .title(st.target.as_ref().map(|t| t.label()).unwrap_or_default())
                .use_markup(false)
                .build();
            let b = gtk::Button::builder().label("Desconectar").valign(gtk::Align::Center).build();
            b.add_css_class("destructive-action");
            let a = self.clone();
            b.connect_clicked(move |_| {
                a.send_cmd(Command::Disconnect);
                a.close_dialog();
            });
            r.add_suffix(&b);
            g.add(&r);
            page.add(&g);
        }

        let mut recent = st.saved.recent.clone();
        if recent.is_empty() {
            recent.extend(st.saved.last_target.clone());
        }
        if !recent.is_empty() {
            let g = adw::PreferencesGroup::builder().title("Conexiones recientes").build();
            for t in recent {
                let r = adw::ActionRow::builder().title(t.label()).use_markup(false).build();
                r.add_suffix(&self.connect_button(t));
                g.add(&r);
            }
            page.add(&g);
        }
        drop(st);

        // Bluetooth
        let ble_group = adw::PreferencesGroup::builder()
            .title("Bluetooth")
            .description(
                "El nodo solo admite un aparato por Bluetooth: desconecta antes el móvil. \
                 La primera vez, empareja el nodo desde los ajustes de Bluetooth del escritorio \
                 (el PIN sale en la pantalla del nodo; si no tiene pantalla suele ser 123456).",
            )
            .build();
        let ble_spinner = gtk::Spinner::builder().spinning(true).build();
        ble_spinner.set_visible(false);
        let scan = gtk::Button::builder().label("Buscar").valign(gtk::Align::Center).build();
        let suffix = gtk::Box::builder().spacing(8).build();
        suffix.append(&ble_spinner);
        suffix.append(&scan);
        ble_group.set_header_suffix(Some(&suffix));
        {
            let a = self.clone();
            let sp = ble_spinner.clone();
            scan.connect_clicked(move |_| {
                sp.set_visible(true);
                a.send_cmd(Command::ScanBle);
            });
        }
        page.add(&ble_group);

        // USB
        let usb = adw::PreferencesGroup::builder()
            .title("USB (puerto serie)")
            .description("La forma más fiable. Si da error de permisos, ver el README.")
            .build();
        let ports_model = gtk::StringList::new(&[]);
        let combo = adw::ComboRow::builder().title("Puerto").model(&ports_model).build();
        let fill_ports = {
            let m = ports_model.clone();
            move || {
                m.splice(0, m.n_items(), &[]);
                let ports = meshtastic::utils::stream::available_serial_ports().unwrap_or_default();
                for p in ports.iter().filter(|p| p.contains("ttyUSB") || p.contains("ttyACM")) {
                    m.append(p);
                }
            }
        };
        fill_ports();
        let refresh = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Buscar puertos")
            .valign(gtk::Align::Center)
            .build();
        refresh.add_css_class("flat");
        refresh.connect_clicked(move |_| fill_ports());
        usb.set_header_suffix(Some(&refresh));
        let usb_btn = gtk::Button::builder().label("Conectar").valign(gtk::Align::Center).build();
        usb_btn.add_css_class("suggested-action");
        {
            let a = self.clone();
            let combo = combo.clone();
            let m = ports_model.clone();
            usb_btn.connect_clicked(move |_| {
                if let Some(port) = m.string(combo.selected()) {
                    a.connect_to(Target::Serial { port: port.to_string() });
                } else {
                    a.toast("No hay ningún puerto USB. Enchufa el nodo y pulsa actualizar.");
                }
            });
        }
        combo.add_suffix(&usb_btn);
        usb.add(&combo);
        page.add(&usb);

        // Red
        let net = adw::PreferencesGroup::builder()
            .title("Red (WiFi o Ethernet)")
            .description("Solo si el nodo tiene la WiFi activada. Puerto 4403 por defecto.")
            .build();
        let host = adw::EntryRow::builder().title("Dirección IP o nombre").build();
        let net_btn = gtk::Button::builder().label("Conectar").valign(gtk::Align::Center).build();
        {
            let a = self.clone();
            let h = host.clone();
            let go = move || {
                let addr = h.text().trim().to_string();
                if !addr.is_empty() {
                    a.connect_to(Target::Tcp { addr });
                }
            };
            let go2 = go.clone();
            net_btn.connect_clicked(move |_| go());
            host.connect_entry_activated(move |_| go2());
        }
        host.add_suffix(&net_btn);
        net.add(&host);
        page.add(&net);

        let header = adw::HeaderBar::new();
        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&header);
        tv.set_content(Some(&page));
        let dialog = adw::Dialog::builder()
            .title("Conectar con un nodo")
            .content_width(560)
            .content_height(680)
            .child(&tv)
            .build();
        {
            let a = self.clone();
            dialog.connect_closed(move |_| {
                a.dialog.borrow_mut().take();
            });
        }
        dialog.present(Some(&self.w.window));
        *self.dialog.borrow_mut() = Some(DialogParts {
            dialog,
            ble_group,
            ble_spinner: ble_spinner.clone(),
            ble_rows: vec![],
        });
        // Empieza a buscar por Bluetooth nada más abrir.
        ble_spinner.set_visible(true);
        self.send_cmd(Command::ScanBle);
    }

    pub(super) fn connect_button(self: &Rc<Self>, t: Target) -> gtk::Button {
        let b = gtk::Button::builder().label("Conectar").valign(gtk::Align::Center).build();
        b.add_css_class("suggested-action");
        let a = self.clone();
        b.connect_clicked(move |_| a.connect_to(t.clone()));
        b
    }

    pub(super) fn connect_to(&self, t: Target) {
        self.send_cmd(Command::Connect(t));
        self.close_dialog();
    }

    pub(super) fn close_dialog(&self) {
        let d = self.dialog.borrow().as_ref().map(|p| p.dialog.clone());
        if let Some(d) = d {
            d.close();
        }
    }

    pub(super) fn show_ble_devices(self: &Rc<Self>, res: Result<Vec<(String, String)>, String>) {
        let mut guard = self.dialog.borrow_mut();
        let Some(p) = guard.as_mut() else { return };
        p.ble_spinner.set_visible(false);
        for r in p.ble_rows.drain(..) {
            p.ble_group.remove(&r);
        }
        let mut rows = vec![];
        match res {
            Err(e) => {
                rows.push(
                    adw::ActionRow::builder()
                        .title("No se pudo buscar")
                        .subtitle(e)
                        .use_markup(false)
                        .build(),
                );
            }
            Ok(mut devs) => {
                devs.retain(|(name, _)| name != "(sin nombre)");
                // Primero los que parecen nodos Meshtastic.
                devs.sort_by_key(|(name, _)| !name.to_lowercase().contains("meshtastic") && !name.contains('_'));
                if devs.is_empty() {
                    rows.push(
                        adw::ActionRow::builder()
                            .title("No se encontró ningún aparato")
                            .subtitle("Comprueba que el nodo está encendido y el móvil desconectado.")
                            .build(),
                    );
                }
                for (name, mac) in devs {
                    let r = adw::ActionRow::builder()
                        .title(name.clone())
                        .subtitle(mac.clone())
                        .use_markup(false)
                        .build();
                    let a = self.clone();
                    let b = gtk::Button::builder().label("Conectar").valign(gtk::Align::Center).build();
                    b.connect_clicked(move |_| a.connect_to(Target::Ble { name: name.clone(), mac: mac.clone() }));
                    r.add_suffix(&b);
                    rows.push(r);
                }
            }
        }
        for r in &rows {
            p.ble_group.add(r);
        }
        p.ble_rows = rows;
    }
}

