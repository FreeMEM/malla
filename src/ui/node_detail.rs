//! Ficha de un nodo: datos, gráficas de telemetría, traceroutes y acciones.

use super::chart::{chart, Series};
use super::*;

const BLUE: (f64, f64, f64) = (0.11, 0.44, 0.85);
const GREEN: (f64, f64, f64) = (0.15, 0.64, 0.41);
const ORANGE: (f64, f64, f64) = (0.90, 0.45, 0.0);
const PURPLE: (f64, f64, f64) = (0.51, 0.24, 0.61);
const RED: (f64, f64, f64) = (0.75, 0.11, 0.16);

/// Periodos del historial: nombre y segundos hacia atrás (0 = todo).
const PERIODS: [(&str, u32); 5] = [("1 hora", 3600), ("24 horas", 86400), ("7 días", 7 * 86400), ("30 días", 30 * 86400), ("Todo", 0)];

fn charts(samples: &[Sample]) -> Vec<gtk::Box> {
    let pts = |f: fn(&Sample) -> Option<f64>| samples.iter().filter_map(|s| f(s).map(|v| (s.t, v))).collect::<Vec<_>>();
    [
        chart("Batería", " %", vec![Series { label: "batería", color: GREEN, points: pts(|s| s.battery.filter(|b| *b <= 100).map(|b| b as f64)) }]),
        chart("Voltaje", " V", vec![Series { label: "voltaje", color: BLUE, points: pts(|s| s.voltage.map(|v| v as f64)) }]),
        chart(
            "Ocupación del aire",
            " %",
            vec![
                Series { label: "canal", color: ORANGE, points: pts(|s| s.ch_util.map(|v| v as f64)) },
                Series { label: "emisión", color: PURPLE, points: pts(|s| s.air_tx.map(|v| v as f64)) },
            ],
        ),
        chart("Señal (SNR)", " dB", vec![Series { label: "SNR", color: BLUE, points: pts(|s| s.snr.map(|v| v as f64)) }]),
        chart("Temperatura", " °C", vec![Series { label: "temp.", color: RED, points: pts(|s| s.temp.map(|v| v as f64)) }]),
        chart("Humedad", " %", vec![Series { label: "humedad", color: BLUE, points: pts(|s| s.hum.map(|v| v as f64)) }]),
        chart("Presión", " hPa", vec![Series { label: "presión", color: PURPLE, points: pts(|s| s.press.map(|v| v as f64)) }]),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Las muestras en una tabla, la más reciente arriba. Solo las columnas con datos.
fn samples_table(samples: &[Sample]) -> gtk::ScrolledWindow {
    type Col = (&'static str, fn(&Sample) -> Option<String>);
    let cols: [Col; 8] = [
        ("Batería", |s| s.battery.map(|b| if b > 100 { "enchufado".into() } else { format!("{b} %") })),
        ("Voltaje", |s| s.voltage.map(|v| format!("{v:.2} V"))),
        ("Canal", |s| s.ch_util.map(|v| format!("{v:.1} %"))),
        ("Emisión", |s| s.air_tx.map(|v| format!("{v:.1} %"))),
        ("SNR", |s| s.snr.map(|v| format!("{v:.1} dB"))),
        ("Temp.", |s| s.temp.map(|v| format!("{v:.1} °C"))),
        ("Humedad", |s| s.hum.map(|v| format!("{v:.0} %"))),
        ("Presión", |s| s.press.map(|v| format!("{v:.1} hPa"))),
    ];
    let used: Vec<&Col> = cols.iter().filter(|c| samples.iter().any(|s| (c.1)(s).is_some())).collect();
    let grid = gtk::Grid::builder().column_spacing(18).row_spacing(4).margin_top(8).margin_bottom(8).margin_start(12).margin_end(12).build();
    let head = |t: &str, c: i32| {
        let l = gtk::Label::builder().label(t).xalign(0.0).build();
        l.add_css_class("heading");
        grid.attach(&l, c, 0, 1, 1);
    };
    head("Fecha", 0);
    for (i, c) in used.iter().enumerate() {
        head(c.0, i as i32 + 1);
    }
    for (r, s) in samples.iter().rev().enumerate() {
        let l = gtk::Label::builder().label(fmt_datetime(s.t)).xalign(0.0).build();
        l.add_css_class("numeric");
        grid.attach(&l, 0, r as i32 + 1, 1, 1);
        for (i, c) in used.iter().enumerate() {
            let l = gtk::Label::builder().label((c.1)(s).unwrap_or_default()).xalign(1.0).build();
            l.add_css_class("numeric");
            grid.attach(&l, i as i32 + 1, r as i32 + 1, 1, 1);
        }
    }
    let sc = gtk::ScrolledWindow::builder().child(&grid).min_content_height(320).max_content_height(420).propagate_natural_height(true).build();
    sc.add_css_class("card");
    sc
}

impl App {
    pub(super) fn open_node_detail(self: &Rc<Self>, num: u32) {
        let st = self.st.borrow();
        let n = st.nodes.get(&num).cloned().unwrap_or(Node { num, ..Default::default() });
        let my = st.my;
        let me = st.my_pos();
        let samples = st.saved.telemetry.get(&num).cloned().unwrap_or_default();
        let traces: Vec<Trace> = st.saved.traces.iter().filter(|t| t.target == num).cloned().collect();
        let track_len = st.saved.tracks.get(&num).map(|t| t.len()).unwrap_or(0);
        let name = st.name_of(num);
        drop(st);

        let page = adw::PreferencesPage::new();

        // Cabecera con avatar.
        let head = gtk::Box::builder().spacing(16).margin_bottom(6).build();
        head.append(&avatar(&n.short(), num, true));
        let tbox = gtk::Box::builder().orientation(gtk::Orientation::Vertical).valign(gtk::Align::Center).build();
        let t = gtk::Label::builder().label(&name).xalign(0.0).build();
        t.add_css_class("title-2");
        let s = gtk::Label::builder()
            .label(format!("{} · {}", Node::id_str(num), if num == my { "este nodo".into() } else { ago(n.last_heard) }))
            .xalign(0.0)
            .build();
        s.add_css_class("dim-label");
        tbox.append(&t);
        tbox.append(&s);
        head.append(&tbox);
        let hg = adw::PreferencesGroup::new();
        hg.add(&head);
        page.add(&hg);

        // Acciones.
        if num != my {
            let g = adw::PreferencesGroup::new();
            let fb = gtk::FlowBox::builder().selection_mode(gtk::SelectionMode::None).max_children_per_line(6).column_spacing(6).row_spacing(6).build();
            let add = |label: &str, f: Box<dyn Fn(&Rc<App>)>| {
                let b = gtk::Button::with_label(label);
                b.add_css_class("pill");
                let a = self.clone();
                b.connect_clicked(move |_| f(&a));
                fb.insert(&b, -1);
            };
            add("Mensaje", Box::new(move |a| a.open_dm(num)));
            add("Traceroute", Box::new(move |a| a.send_cmd(Command::Traceroute(num))));
            add("Pedir posición", Box::new(move |a| a.send_cmd(Command::RequestPosition(num))));
            add("Intercambiar datos", Box::new(move |a| a.send_cmd(Command::RequestUser(num))));
            if n.lat.is_some() {
                add("Ver en el mapa", Box::new(move |a| a.show_node_on_map(num)));
            }
            if track_len > 1 {
                add("Ver rastro", Box::new(move |a| a.show_track(num)));
            }
            let fav = n.favorite;
            add(if fav { "Quitar favorito" } else { "Favorito" }, Box::new(move |a| a.set_favorite(num, !fav)));
            let ign = n.ignored;
            add(if ign { "Dejar de ignorar" } else { "Ignorar" }, Box::new(move |a| a.set_ignored(num, !ign)));
            g.add(&fb);
            page.add(&g);
        }

        // Historial: gráficas o tabla, con periodo.
        let g = adw::PreferencesGroup::builder()
            .title("Historial")
            .description(if samples.is_empty() {
                "Aún no hay telemetría guardada de este nodo. Se va recogiendo mientras Malla está conectada."
            } else {
                "Telemetría recogida mientras Malla estaba conectada."
            })
            .build();
        if !samples.is_empty() {
            let period = gtk::DropDown::from_strings(&PERIODS.iter().map(|p| p.0).collect::<Vec<_>>());
            period.set_selected(PERIODS.len() as u32 - 1);
            period.set_valign(gtk::Align::Center);
            let table = gtk::ToggleButton::builder().label("Tabla").valign(gtk::Align::Center).build();
            let suffix = gtk::Box::builder().spacing(6).build();
            suffix.append(&period);
            suffix.append(&table);
            g.set_header_suffix(Some(&suffix));
            let holder = gtk::Box::builder().orientation(gtk::Orientation::Vertical).build();
            let fill = {
                let holder = holder.clone();
                let samples = samples.clone();
                let period = period.clone();
                let table = table.clone();
                move || {
                    while let Some(c) = holder.first_child() {
                        holder.remove(&c);
                    }
                    let since = PERIODS.get(period.selected() as usize).map(|p| p.1).unwrap_or(0);
                    let from = if since == 0 { 0 } else { now().saturating_sub(since) };
                    let sel: Vec<Sample> = samples.iter().filter(|s| s.t >= from).cloned().collect();
                    let w: gtk::Widget = if sel.is_empty() {
                        let l = gtk::Label::builder().label("No hay datos en este periodo.").margin_top(12).margin_bottom(12).build();
                        l.add_css_class("dim-label");
                        l.upcast()
                    } else if table.is_active() {
                        samples_table(&sel).upcast()
                    } else {
                        let cbox = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).build();
                        for c in charts(&sel) {
                            cbox.append(&c);
                        }
                        cbox.upcast()
                    };
                    holder.append(&w);
                }
            };
            fill();
            let f = std::rc::Rc::new(fill);
            let f2 = f.clone();
            period.connect_selected_notify(move |_| f2());
            table.connect_toggled(move |_| f());
            g.add(&holder);
        }
        page.add(&g);

        // Estadísticas de la radio (solo tu nodo).
        if num == my {
            if let Some((t, ls)) = self.st.borrow().local_stats.clone() {
                let g = adw::PreferencesGroup::builder()
                    .title("Estadísticas de la radio")
                    .description(format!("Las manda tu nodo de vez en cuando. Últimas: {}.", ago(t)))
                    .build();
                let pct = |a: u32, b: u32| if b > 0 { format!(" ({:.0} %)", a as f64 * 100.0 / b as f64) } else { String::new() };
                let rows = [
                    ("Paquetes enviados", ls.num_packets_tx.to_string()),
                    ("Paquetes recibidos", ls.num_packets_rx.to_string()),
                    ("Recibidos con errores", format!("{}{}", ls.num_packets_rx_bad, pct(ls.num_packets_rx_bad, ls.num_packets_rx))),
                    ("Recibidos repetidos", format!("{}{}", ls.num_rx_dupe, pct(ls.num_rx_dupe, ls.num_packets_rx))),
                    ("Retransmitidos para otros", ls.num_tx_relay.to_string()),
                    ("Retransmisiones evitadas", ls.num_tx_relay_canceled.to_string()),
                    ("Descartados al enviar", ls.num_tx_dropped.to_string()),
                    ("Nodos activos / conocidos", format!("{} / {}", ls.num_online_nodes, ls.num_total_nodes)),
                    ("Uso del canal", format!("{:.1} %", ls.channel_utilization)),
                    ("Tiempo de emisión", format!("{:.1} %", ls.air_util_tx)),
                    ("Encendido desde hace", fmt_duration(ls.uptime_seconds)),
                ];
                for (k, v) in rows {
                    let r = adw::ActionRow::builder().title(k).subtitle(v).use_markup(false).build();
                    r.add_css_class("property");
                    g.add(&r);
                }
                page.add(&g);
            }
        }

        // Datos.
        let g = adw::PreferencesGroup::builder().title("Datos").build();
        let row = |t: &str, v: String| {
            if v.is_empty() {
                return;
            }
            let r = adw::ActionRow::builder().title(t).subtitle(v).use_markup(false).build();
            r.add_css_class("property");
            g.add(&r);
        };
        row("Nombre corto", n.short_name.clone());
        row("Modelo", n.hw_model.clone());
        row("Función", n.role.clone());
        row("Último contacto", if n.last_heard > 0 { fmt_datetime(n.last_heard) } else { "nunca".into() });
        row(
            "Ruta",
            if n.via_mqtt { "por MQTT (Internet)".into() } else { n.hops_away.map(hops_text).unwrap_or_default() },
        );
        row("SNR", n.snr.map(|v| format!("{v:.2} dB")).unwrap_or_default());
        row("RSSI", n.rssi.map(|v| format!("{v} dBm")).unwrap_or_default());
        row("Batería", n.battery.map(|b| if b > 100 { "enchufado".into() } else { format!("{b} %") }).unwrap_or_default());
        row("Voltaje", n.voltage.map(|v| format!("{v:.2} V")).unwrap_or_default());
        row("Uso del canal", n.channel_util.map(|v| format!("{v:.1} %")).unwrap_or_default());
        row("Tiempo de emisión", n.air_util_tx.map(|v| format!("{v:.1} %")).unwrap_or_default());
        row("Encendido desde hace", n.uptime.map(fmt_duration).unwrap_or_default());
        row("Temperatura", n.temperature.map(|v| format!("{v:.1} °C")).unwrap_or_default());
        row("Humedad", n.humidity.map(|v| format!("{v:.0} %")).unwrap_or_default());
        row("Presión", n.pressure.map(|v| format!("{v:.1} hPa")).unwrap_or_default());
        if let (Some(la), Some(lo)) = (n.lat, n.lon) {
            let alt = n.alt.map(|a| format!(", {a} m")).unwrap_or_default();
            row("Posición", format!("{la:.5}, {lo:.5}{alt}"));
            if let Some(m) = me.filter(|_| num != my) {
                let (d, b) = distance_bearing(m, (la, lo));
                row("Distancia", format!("{} al {} ({b:.0}°)", fmt_distance(d), compass(b)));
            }
        }
        row("Cifrado directo", if n.has_pubkey { "sí, tiene clave pública".into() } else { "clave desconocida".into() });
        if n.licensed {
            row("Radioaficionado", "sí (sin cifrado)".into());
        }
        page.add(&g);

        // Traceroutes.
        if !traces.is_empty() {
            let g = adw::PreferencesGroup::builder().title("Traceroutes").build();
            for t in traces.iter().rev().take(10) {
                let r = adw::ActionRow::builder()
                    .title(format!("{} · {} saltos", fmt_datetime(t.time), t.towards.len() + 1))
                    .subtitle(self.route_text(t, false))
                    .subtitle_lines(2)
                    .use_markup(false)
                    .activatable(true)
                    .build();
                let a = self.clone();
                let tt = t.clone();
                r.connect_activated(move |_| a.show_trace(&tt));
                g.add(&r);
            }
            page.add(&g);
        }

        let header = adw::HeaderBar::new();
        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&header);
        tv.set_content(Some(&page));
        let dialog = adw::Dialog::builder().title(&name).content_width(720).content_height(760).child(&tv).build();
        dialog.present(Some(&self.w.window));
    }

    fn route_text(&self, t: &Trace, back: bool) -> String {
        let st = self.st.borrow();
        let snr = |v: &Vec<Option<f32>>, i: usize| v.get(i).copied().flatten().map(|s| format!(" ({s:.1} dB)")).unwrap_or_default();
        let (start, hops, snrs, end) = if back { (t.target, &t.back, &t.snr_back, st.my) } else { (st.my, &t.towards, &t.snr_towards, t.target) };
        let mut s = st.short_of(start);
        for (i, h) in hops.iter().enumerate() {
            s += &format!(" → {}{}", st.short_of(*h), snr(snrs, i));
        }
        s += &format!(" → {}{}", st.short_of(end), snr(snrs, hops.len()));
        s
    }

    pub(super) fn show_trace(self: &Rc<Self>, t: &Trace) {
        let page = adw::PreferencesPage::new();
        let name = self.st.borrow().name_of(t.target);
        let mk = |title: &str, start: u32, hops: &Vec<u32>, snrs: &Vec<Option<f32>>, end: u32| {
            let g = adw::PreferencesGroup::builder().title(title).build();
            let st = self.st.borrow();
            let mut nodes = vec![start];
            nodes.extend(hops.iter().copied());
            nodes.push(end);
            for (i, n) in nodes.iter().enumerate() {
                let sub = if i == 0 {
                    "origen".to_string()
                } else {
                    let s = snrs.get(i - 1).copied().flatten().map(|v| format!("recibido con SNR {v:.1} dB")).unwrap_or_else(|| "SNR desconocida".into());
                    format!("salto {i} · {s}")
                };
                let r = adw::ActionRow::builder().title(st.name_of(*n)).subtitle(sub).use_markup(false).build();
                r.add_prefix(&avatar(&st.short_of(*n), *n, false));
                g.add(&r);
            }
            g
        };
        let my = self.st.borrow().my;
        page.add(&mk("Ida", my, &t.towards, &t.snr_towards, t.target));
        if t.has_back {
            page.add(&mk("Vuelta", t.target, &t.back, &t.snr_back, my));
        }
        let header = adw::HeaderBar::new();
        let on_map = gtk::Button::builder().label("Ver en el mapa").build();
        header.pack_start(&on_map);
        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&header);
        tv.set_content(Some(&page));
        let dialog = adw::Dialog::builder()
            .title(format!("Traceroute a {name}"))
            .content_width(520)
            .content_height(620)
            .child(&tv)
            .build();
        let a = self.clone();
        let d = dialog.clone();
        let tt = t.clone();
        on_map.connect_clicked(move |_| {
            d.force_close();
            a.show_trace_on_map(&tt);
        });
        dialog.present(Some(&self.w.window));
    }
}
