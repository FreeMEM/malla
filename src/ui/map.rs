//! Mapa: nodos, waypoints, rastros y capas.

use super::*;

pub(super) struct MapUi {
    pub root: gtk::Overlay,
    pub map: shumate::SimpleMap,
    pub markers: shumate::MarkerLayer,
    pub wp_markers: shumate::MarkerLayer,
    pub clear_track: gtk::Button,
    pub measure_label: gtk::Label,
    pub extra: MapExtra,
}

pub(super) struct MapExtra {
    center: gtk::Button,
    fit: gtk::Button,
    layer: gtk::DropDown,
    night: gtk::ToggleButton,
    search: gtk::SearchEntry,
}

/// (nombre, id, url de teselas, licencia, zoom máximo)
const LAYERS: [(&str, &str, &str, &str, u32); 3] = [
    ("Calles", "osm", "", "", 19),
    (
        "Topográfico",
        "opentopomap",
        "https://tile.opentopomap.org/{z}/{x}/{y}.png",
        "Map data © OpenStreetMap contributors, SRTM · Map style © OpenTopoMap (CC-BY-SA)",
        17,
    ),
    (
        "Humanitario",
        "hot",
        "https://a.tile.openstreetmap.fr/hot/{z}/{x}/{y}.png",
        "© OpenStreetMap contributors · Tiles style by Humanitarian OpenStreetMap Team hosted by OSM France",
        19,
    ),
];

fn make_source(i: usize) -> Option<shumate::MapSource> {
    let (name, id, url, lic, max) = LAYERS.get(i).copied()?;
    if url.is_empty() {
        return shumate::MapSourceRegistry::with_defaults().by_id(shumate::MAP_SOURCE_OSM_MAPNIK);
    }
    let r = shumate::RasterRenderer::from_url(url);
    r.set_id(id);
    r.set_name(name);
    r.set_license(lic);
    r.set_max_zoom_level(max);
    r.set_min_zoom_level(0);
    Some(r.upcast())
}

pub(super) fn build_page() -> MapUi {
    let map = shumate::SimpleMap::new();
    if let Some(src) = make_source(0) {
        map.set_map_source(Some(&src));
    }
    map.set_vexpand(true);
    let viewport = map.viewport().expect("el mapa no tiene viewport");
    let wp_markers = shumate::MarkerLayer::new(&viewport);
    let markers = shumate::MarkerLayer::new(&viewport);
    if let Some(m) = map.map() {
        m.add_layer(&wp_markers);
        m.add_layer(&markers);
        m.center_on(40.0, -3.7);
    }
    viewport.set_zoom_level(5.0);

    let center = gtk::Button::builder().label("Mi nodo").tooltip_text("Centrar el mapa en mi nodo").build();
    center.add_css_class("mapbtn");
    let names: Vec<&str> = LAYERS.iter().map(|l| l.0).collect();
    let layer = gtk::DropDown::from_strings(&names);
    layer.set_tooltip_text(Some("Tipo de mapa"));
    layer.add_css_class("mapbtn");
    let fit = gtk::Button::builder().label("Todos").tooltip_text("Ver todos los nodos con posición").build();
    fit.add_css_class("mapbtn");
    let night = gtk::ToggleButton::builder().icon_name("weather-clear-night-symbolic").tooltip_text("Mapa oscuro").build();
    night.add_css_class("mapbtn");
    let search = gtk::SearchEntry::builder().placeholder_text("Buscar nodo o punto").width_chars(18).build();
    let clear_track = gtk::Button::builder().label("Quitar rastro").visible(false).build();
    clear_track.add_css_class("mapbtn");
    let tools = gtk::Box::builder()
        .spacing(6)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_start(12)
        .margin_top(12)
        .build();
    tools.add_css_class("osd");
    tools.add_css_class("toolbar");
    tools.append(&center);
    tools.append(&fit);
    tools.append(&layer);
    tools.append(&night);
    tools.append(&search);
    tools.append(&clear_track);
    let hint = gtk::Label::new(Some("Clic derecho en el mapa para añadir un punto"));
    hint.add_css_class("meta");
    hint.add_css_class("osd");
    hint.set_halign(gtk::Align::Center);
    hint.set_valign(gtk::Align::End);
    hint.set_margin_bottom(36);

    // Resultado de medir una distancia, arriba a la derecha.
    let measure_label = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Start)
        .margin_top(72)
        .visible(false)
        .build();
    measure_label.add_css_class("osd");
    measure_label.add_css_class("measure");

    let root = gtk::Overlay::new();
    root.set_child(Some(&map));
    root.add_overlay(&tools);
    root.add_overlay(&hint);
    root.add_overlay(&measure_label);
    MapUi { root, map, markers, wp_markers, clear_track, measure_label, extra: MapExtra { center, fit, layer, night, search } }
}

pub(super) fn connect_signals(app: &Rc<App>, x: &MapExtra) {
    let a = app.clone();
    x.center.connect_clicked(move |_| {
        let me = a.st.borrow().my_pos();
        match me {
            Some((la, lo)) => {
                if let Some(m) = a.w.map.map() {
                    m.go_to_full(la, lo, 13.0);
                }
            }
            None => a.toast("Tu nodo no tiene posición. Puedes fijarla en Ajustes > Posición fija."),
        }
    });

    let a = app.clone();
    x.fit.connect_clicked(move |_| {
        let pts: Vec<(f64, f64)> = a.st.borrow().nodes.values().filter(|n| !n.ignored).filter_map(|n| Some((n.lat?, n.lon?))).collect();
        if pts.is_empty() {
            a.toast("Ningún nodo ha compartido su posición todavía");
        } else {
            a.fit_map(&pts);
        }
    });

    let saved = app.st.borrow().saved.prefs.map_source.clone();
    if let Some(i) = LAYERS.iter().position(|l| l.1 == saved) {
        x.layer.set_selected(i as u32);
        if let Some(src) = make_source(i) {
            app.w.map.set_map_source(Some(&src));
        }
    }
    let night = x.night.clone();
    let a = app.clone();
    x.layer.connect_selected_notify(move |d| {
        let i = d.selected() as usize;
        if let Some(src) = make_source(i) {
            a.w.map.set_map_source(Some(&src));
            a.st.borrow_mut().saved.prefs.map_source = LAYERS[i].1.to_string();
            a.save();
            // La capa base es nueva: hay que volver a oscurecerla.
            set_night(&a.w.map, night.is_active());
        }
    });

    let on = app.st.borrow().saved.prefs.map_night;
    x.night.set_active(on);
    set_night(&app.w.map, on);
    let a = app.clone();
    x.night.connect_toggled(move |b| {
        set_night(&a.w.map, b.is_active());
        a.st.borrow_mut().saved.prefs.map_night = b.is_active();
        a.save();
    });

    let a = app.clone();
    x.search.connect_activate(move |e| {
        let q = e.text().trim().to_lowercase();
        if q.is_empty() {
            return;
        }
        let found = {
            let st = a.st.borrow();
            let node = st
                .nodes
                .values()
                .filter(|n| n.lat.is_some())
                .find(|n| n.long_name.to_lowercase().contains(&q) || n.short_name.to_lowercase().contains(&q) || Node::id_str(n.num).contains(&q))
                .and_then(|n| Some((n.lat?, n.lon?)));
            node.or_else(|| st.saved.waypoints.values().find(|w| w.name.to_lowercase().contains(&q)).map(|w| (w.lat, w.lon)))
        };
        match found {
            Some((la, lo)) => a.show_on_map(la, lo),
            None => a.toast("No encuentro ningún nodo con posición ni punto con ese nombre"),
        }
    });

    // Clic izquierdo: termina una medida empezada desde el menú.
    let a = app.clone();
    let g = gtk::GestureClick::builder().button(1).build();
    g.connect_released(move |g, n, x, y| {
        let Some(from) = a.st.borrow().measure_from else { return };
        if n != 1 {
            return;
        }
        let Some(w) = g.widget() else { return };
        let Some(vp) = a.w.map.viewport() else { return };
        let to = vp.widget_coords_to_location(&w, x, y);
        a.st.borrow_mut().measure_from = None;
        a.draw_measure(from, to);
    });
    app.w.map.add_controller(g);

    let a = app.clone();
    let g = gtk::GestureClick::new();
    g.connect_released(move |_, _, _, _| a.clear_measure());
    app.w.measure_label.add_controller(g);

    let a = app.clone();
    app.w.clear_track.connect_clicked(move |b| {
        a.st.borrow_mut().track_of = None;
        if let Some(l) = a.w.track_layer.borrow_mut().take() {
            if let Some(m) = a.w.map.map() {
                m.remove_layer(&l);
            }
        }
        b.set_visible(false);
    });

    // Clic derecho: añadir punto.
    let a = app.clone();
    let g = gtk::GestureClick::builder().button(3).build();
    g.connect_pressed(move |g, _, x, y| {
        let Some(w) = g.widget() else { return };
        let Some(vp) = a.w.map.viewport() else { return };
        let (la, lo) = vp.widget_coords_to_location(&w, x, y);
        a.map_context_menu(&w, x, y, la, lo);
    });
    app.w.map.add_controller(g);
}

impl App {
    pub(super) fn show_on_map(&self, la: f64, lo: f64) {
        self.w.stack.set_visible_child_name("mapa");
        self.map_centered.set(true);
        if let Some(m) = self.w.map.map() {
            m.go_to_full(la, lo, 14.0);
        }
    }

    /// Encuadra el mapa para que se vean todos estos puntos.
    pub(super) fn fit_map(&self, pts: &[(f64, f64)]) {
        let Some(m) = self.w.map.map() else { return };
        let (mut a0, mut a1, mut o0, mut o1) = (90.0f64, -90.0f64, 180.0f64, -180.0f64);
        for (la, lo) in pts {
            a0 = a0.min(*la);
            a1 = a1.max(*la);
            o0 = o0.min(*lo);
            o1 = o1.max(*lo);
        }
        let (cla, clo) = ((a0 + a1) / 2.0, (o0 + o1) / 2.0);
        let span = (a1 - a0).max((o1 - o0) * cla.to_radians().cos()).max(1e-4);
        let zoom = ((360.0 / span).log2() - 1.5).clamp(3.0, 15.0);
        self.w.stack.set_visible_child_name("mapa");
        self.map_centered.set(true);
        m.go_to_full(cla, clo, zoom);
    }

    /// Dibuja una línea sobre el mapa en la capa de rastros (rastro o ruta).
    fn draw_line(&self, pts: &[(f64, f64)], what: &str) {
        let Some(vp) = self.w.map.viewport() else { return };
        let Some(m) = self.w.map.map() else { return };
        if let Some(l) = self.w.track_layer.borrow_mut().take() {
            m.remove_layer(&l);
        }
        let layer = shumate::PathLayer::new(&vp);
        layer.set_stroke_color(Some(&gdk::RGBA::new(0.78, 0.27, 0.0, 0.9)));
        layer.set_stroke_width(4.0);
        for (la, lo) in pts {
            layer.add_node(&shumate::Coordinate::new_full(*la, *lo));
        }
        m.insert_layer_behind(&layer, Some(&self.w.wp_markers));
        *self.w.track_layer.borrow_mut() = Some(layer);
        self.w.clear_track.set_label(&format!("Quitar {what}"));
        self.w.clear_track.set_visible(true);
        self.fit_map(pts);
    }

    /// Dibuja en el mapa la ruta de un traceroute, con los nodos que tienen posición.
    pub(super) fn show_trace_on_map(&self, t: &Trace) {
        let (pts, missing) = {
            let st = self.st.borrow();
            let mut nodes = vec![st.my];
            nodes.extend(t.towards.iter().copied());
            nodes.push(t.target);
            if t.has_back {
                nodes.extend(t.back.iter().copied());
                nodes.push(st.my);
            }
            let mut pts = vec![];
            let mut missing = vec![];
            for n in nodes {
                match st.nodes.get(&n).and_then(|x| Some((x.lat?, x.lon?))) {
                    Some(p) => pts.push(p),
                    None => {
                        let s = st.short_of(n);
                        if !missing.contains(&s) {
                            missing.push(s);
                        }
                    }
                }
            }
            (pts, missing)
        };
        if pts.len() < 2 {
            self.toast("No hay posiciones suficientes para dibujar esta ruta");
            return;
        }
        // Cierra la ficha y demás ventanas que tapen el mapa.
        for _ in 0..5 {
            match self.w.window.visible_dialog() {
                Some(d) => {
                    d.force_close();
                }
                None => break,
            }
        }
        self.st.borrow_mut().track_of = None;
        self.draw_line(&pts, "ruta");
        if !missing.is_empty() {
            self.toast(&format!("Sin posición, no salen en la línea: {}", missing.join(", ")));
        }
    }

    pub(super) fn draw_measure(&self, from: (f64, f64), to: (f64, f64)) {
        let Some(vp) = self.w.map.viewport() else { return };
        let Some(m) = self.w.map.map() else { return };
        if let Some(l) = self.w.measure_layer.borrow_mut().take() {
            m.remove_layer(&l);
        }
        let layer = shumate::PathLayer::new(&vp);
        layer.set_stroke_color(Some(&gdk::RGBA::new(0.11, 0.44, 0.85, 0.95)));
        layer.set_stroke_width(3.0);
        layer.add_node(&shumate::Coordinate::new_full(from.0, from.1));
        layer.add_node(&shumate::Coordinate::new_full(to.0, to.1));
        m.add_layer(&layer);
        *self.w.measure_layer.borrow_mut() = Some(layer);
        let (d, b) = distance_bearing(from, to);
        self.w.measure_label.set_text(&format!("{} al {} ({b:.0}°)  ✕", fmt_distance(d), compass(b)));
        self.w.measure_label.set_tooltip_text(Some("Pulsa para quitar la medida"));
        self.w.measure_label.set_visible(true);
    }

    fn clear_measure(&self) {
        self.st.borrow_mut().measure_from = None;
        if let Some(l) = self.w.measure_layer.borrow_mut().take() {
            if let Some(m) = self.w.map.map() {
                m.remove_layer(&l);
            }
        }
        self.w.measure_label.set_visible(false);
    }

    pub(super) fn show_node_on_map(&self, num: u32) {
        let pos = self.st.borrow().nodes.get(&num).and_then(|n| Some((n.lat?, n.lon?)));
        if let Some((la, lo)) = pos {
            self.show_on_map(la, lo);
        }
    }

    /// Dibuja el rastro de posiciones guardadas de un nodo.
    pub(super) fn show_track(&self, num: u32) {
        let pts = self.st.borrow().saved.tracks.get(&num).cloned().unwrap_or_default();
        if pts.len() < 2 {
            self.toast("Aún no hay suficientes posiciones guardadas de este nodo para dibujar un rastro.");
            return;
        }
        let line: Vec<(f64, f64)> = pts.iter().map(|p| (p.1, p.2)).collect();
        self.draw_line(&line, "rastro");
        self.st.borrow_mut().track_of = Some(num);
        self.toast(&format!("Rastro de {} posiciones", pts.len()));
    }

    fn map_context_menu(self: &Rc<Self>, w: &gtk::Widget, x: f64, y: f64, la: f64, lo: f64) {
        let pop = gtk::Popover::new();
        pop.set_parent(w);
        pop.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        let bx = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).build();
        let l = gtk::Label::new(Some(&format!("{la:.5}, {lo:.5}")));
        l.add_css_class("meta");
        bx.append(&l);
        let add = gtk::Button::with_label("Añadir punto aquí…");
        add.add_css_class("flat");
        let copy = gtk::Button::with_label("Copiar coordenadas");
        copy.add_css_class("flat");
        let fixed = gtk::Button::with_label("Fijar aquí la posición de mi nodo…");
        fixed.add_css_class("flat");
        let measure = gtk::Button::with_label("Medir distancia desde aquí");
        measure.add_css_class("flat");
        for b in [&add, &copy, &measure, &fixed] {
            b.child().and_downcast::<gtk::Label>().inspect(|l| l.set_xalign(0.0));
            bx.append(b);
        }
        let a = self.clone();
        let p = pop.clone();
        measure.connect_clicked(move |_| {
            p.popdown();
            a.clear_measure();
            a.st.borrow_mut().measure_from = Some((la, lo));
            a.w.measure_label.set_text("Pulsa en el mapa el otro punto");
            a.w.measure_label.set_visible(true);
        });
        pop.set_child(Some(&bx));
        let a = self.clone();
        let p = pop.clone();
        add.connect_clicked(move |_| {
            p.popdown();
            a.waypoint_dialog(la, lo, None);
        });
        let p = pop.clone();
        let ww = w.clone();
        copy.connect_clicked(move |_| {
            p.popdown();
            ww.clipboard().set_text(&format!("{la:.6}, {lo:.6}"));
        });
        let a = self.clone();
        let p = pop.clone();
        fixed.connect_clicked(move |_| {
            p.popdown();
            let a2 = a.clone();
            a.confirm(
                "¿Fijar la posición del nodo?",
                &format!("Tu nodo anunciará {la:.5}, {lo:.5} como su posición y dejará de usar el GPS."),
                "Fijar",
                move || a2.set_fixed_position(la, lo, 0),
            );
        });
        pop.popup();
    }

    pub(super) fn set_fixed_position(&self, la: f64, lo: f64, alt: i32) {
        self.admin(vec![protobufs::admin_message::PayloadVariant::SetFixedPosition(protobufs::Position {
            latitude_i: Some((la * 1e7) as i32),
            longitude_i: Some((lo * 1e7) as i32),
            altitude: Some(alt),
            time: now(),
            ..Default::default()
        })]);
        let my = self.st.borrow().my;
        if let Some(n) = self.st.borrow_mut().nodes.get_mut(&my) {
            n.lat = Some(la);
            n.lon = Some(lo);
            n.alt = Some(alt);
        }
        self.nodes_dirty.set(true);
    }

    /// Crear o editar un waypoint.
    pub(super) fn waypoint_dialog(self: &Rc<Self>, la: f64, lo: f64, existing: Option<Waypoint>) {
        let page = adw::PreferencesPage::new();
        let g = adw::PreferencesGroup::builder().description(format!("{la:.5}, {lo:.5}")).build();
        let name = adw::EntryRow::builder().title("Nombre").build();
        let desc = adw::EntryRow::builder().title("Descripción").build();
        let icon = adw::EntryRow::builder().title("Icono (un emoji)").text("📍").build();
        let exp_model = gtk::StringList::new(&["Nunca", "1 hora", "8 horas", "1 día", "1 semana"]);
        let exp = adw::ComboRow::builder().title("Caduca").model(&exp_model).build();
        let lock = adw::SwitchRow::builder().title("Solo yo puedo cambiarlo").build();
        if let Some(w) = &existing {
            name.set_text(&w.name);
            desc.set_text(&w.description);
            icon.set_text(&w.icon_str());
        }
        for r in [name.upcast_ref::<gtk::Widget>(), desc.upcast_ref(), icon.upcast_ref(), exp.upcast_ref(), lock.upcast_ref()] {
            g.add(r);
        }
        page.add(&g);

        let send = gtk::Button::builder().label("Enviar a la malla").build();
        send.add_css_class("suggested-action");
        let header = adw::HeaderBar::new();
        header.pack_end(&send);
        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&header);
        tv.set_content(Some(&page));
        let dialog = adw::Dialog::builder()
            .title(if existing.is_some() { "Editar punto" } else { "Nuevo punto en el mapa" })
            .content_width(460)
            .child(&tv)
            .build();
        let a = self.clone();
        let d = dialog.clone();
        let id = existing.as_ref().map(|w| w.id).unwrap_or_else(|| meshtastic::utils::generate_rand_id::<u32>());
        send.connect_clicked(move |_| {
            let n = name.text().trim().to_string();
            if n.is_empty() {
                a.toast("Ponle un nombre al punto");
                return;
            }
            let secs = [0u32, 3600, 8 * 3600, 86400, 7 * 86400][exp.selected() as usize % 5];
            let my = a.st.borrow().my;
            let icon_cp = icon.text().chars().next().map(|c| c as u32).unwrap_or(0);
            let w = protobufs::Waypoint {
                id,
                latitude_i: Some((la * 1e7) as i32),
                longitude_i: Some((lo * 1e7) as i32),
                expire: if secs == 0 { 0 } else { now() + secs },
                locked_to: if lock.is_active() { my } else { 0 },
                name: n.chars().take(29).collect(),
                description: desc.text().chars().take(99).collect(),
                icon: icon_cp,
                ..Default::default()
            };
            if !a.st.borrow().connected {
                a.toast("No hay conexión con el nodo");
                return;
            }
            a.send_cmd(Command::SendWaypoint(w.clone()));
            a.st.borrow_mut().saved.waypoints.insert(
                id,
                Waypoint {
                    id,
                    lat: la,
                    lon: lo,
                    name: w.name.clone(),
                    description: w.description.clone(),
                    icon: icon_cp,
                    expire: w.expire,
                    locked_to: w.locked_to,
                    from: my,
                    time: now(),
                },
            );
            a.save();
            a.rebuild_waypoints();
            d.close();
        });
        dialog.present(Some(&self.w.window));
    }

    fn delete_waypoint(self: &Rc<Self>, w: &Waypoint) {
        if self.st.borrow().connected {
            // Caducado = borrado para el resto de la malla.
            self.send_cmd(Command::SendWaypoint(protobufs::Waypoint {
                id: w.id,
                latitude_i: Some((w.lat * 1e7) as i32),
                longitude_i: Some((w.lon * 1e7) as i32),
                expire: 1,
                name: w.name.clone(),
                ..Default::default()
            }));
        }
        self.st.borrow_mut().saved.waypoints.remove(&w.id);
        self.save();
        self.rebuild_waypoints();
    }

    pub(super) fn rebuild_waypoints(self: &Rc<Self>) {
        self.w.wp_markers.remove_all();
        let t = now();
        let wps: Vec<Waypoint> = {
            let mut st = self.st.borrow_mut();
            st.saved.waypoints.retain(|_, w| w.expire == 0 || w.expire > t);
            st.saved.waypoints.values().cloned().collect()
        };
        for w in wps {
            let st = self.st.borrow();
            let bx = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(4)
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(6)
                .margin_end(6)
                .build();
            let title = gtk::Label::builder()
                .use_markup(true)
                .label(format!("<b>{} {}</b>", w.icon_str(), glib::markup_escape_text(&w.name)))
                .xalign(0.0)
                .build();
            bx.append(&title);
            let mut lines = vec![];
            if !w.description.is_empty() {
                lines.push(w.description.clone());
            }
            lines.push(format!("De {}", if w.from == st.my { "ti".to_string() } else { st.name_of(w.from) }));
            lines.push(if w.expire == 0 { "No caduca".into() } else { format!("Caduca {}", fmt_datetime(w.expire)) });
            if let Some(me) = st.my_pos() {
                let (d, b) = distance_bearing(me, (w.lat, w.lon));
                lines.push(format!("A {} al {}", fmt_distance(d), compass(b)));
            }
            let info = gtk::Label::builder().label(lines.join("\n")).xalign(0.0).wrap(true).max_width_chars(40).build();
            info.add_css_class("meta");
            bx.append(&info);
            let can_edit = w.locked_to == 0 || w.locked_to == st.my;
            drop(st);
            let pop = gtk::Popover::builder().child(&bx).build();
            if can_edit {
                let row = gtk::Box::builder().spacing(6).build();
                let edit = gtk::Button::with_label("Editar");
                let del = gtk::Button::with_label("Borrar");
                del.add_css_class("destructive-action");
                row.append(&edit);
                row.append(&del);
                bx.append(&row);
                let a = self.clone();
                let ww = w.clone();
                let p = pop.clone();
                edit.connect_clicked(move |_| {
                    p.popdown();
                    a.waypoint_dialog(ww.lat, ww.lon, Some(ww.clone()));
                });
                let a = self.clone();
                let ww = w.clone();
                let p = pop.clone();
                del.connect_clicked(move |_| {
                    p.popdown();
                    a.delete_waypoint(&ww);
                });
            }
            let mb = gtk::MenuButton::builder()
                .child(&gtk::Label::new(Some(&w.icon_str())))
                .popover(&pop)
                .tooltip_text(&w.name)
                .build();
            mb.add_css_class("mapmarker");
            mb.add_css_class("wp");
            let marker = shumate::Marker::builder().child(&mb).build();
            marker.set_location(w.lat, w.lon);
            self.w.wp_markers.add_marker(&marker);
        }
    }

    pub(super) fn rebuild_map(self: &Rc<Self>) {
        let st = self.st.borrow();
        let my = st.my;
        let t = now();
        let mut items = vec![];
        for n in st.nodes.values() {
            let (Some(la), Some(lo)) = (n.lat, n.lon) else { continue };
            if n.ignored {
                continue;
            }
            let old = n.num != my && t.saturating_sub(n.last_heard) > 2 * 3600;
            items.push((n.num, la, lo, n.short(), old, n.favorite));
        }
        // Solo se redibuja si algo cambió, para no cerrar una ficha abierta.
        let sig = format!("{my}{items:?}");
        if *self.map_sig.borrow() == sig {
            return;
        }
        *self.map_sig.borrow_mut() = sig;

        self.w.markers.remove_all();
        let me = st.my_pos();
        for (num, la, lo, label, old, fav) in &items {
            let n = &st.nodes[num];
            let pop = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(4)
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(6)
                .margin_end(6)
                .build();
            let title = gtk::Label::builder()
                .use_markup(true)
                .label(format!("<b>{}</b>", glib::markup_escape_text(&n.display_name())))
                .xalign(0.0)
                .build();
            pop.append(&title);
            let mut lines = vec![Node::id_str(*num)];
            if *num == my {
                lines.push("este nodo".into());
            } else {
                lines.push(format!("oído {}", ago(n.last_heard)));
                if let Some(h) = n.hops_away {
                    lines.push(hops_text(h));
                }
                if let Some(m) = me {
                    let (d, b) = distance_bearing(m, (*la, *lo));
                    lines.push(format!("a {} al {}", fmt_distance(d), compass(b)));
                }
            }
            if !n.hw_model.is_empty() {
                lines.push(n.hw_model.clone());
            }
            if let Some(b) = n.battery {
                lines.push(battery_text(b));
            }
            let info = gtk::Label::builder().label(lines.join("\n")).xalign(0.0).build();
            info.add_css_class("meta");
            pop.append(&info);
            let popover = gtk::Popover::builder().child(&pop).build();
            let row = gtk::Box::builder().spacing(6).build();
            let ficha = gtk::Button::with_label("Ficha");
            row.append(&ficha);
            let a = self.clone();
            let num_c = *num;
            let p = popover.clone();
            ficha.connect_clicked(move |_| {
                p.popdown();
                a.open_node_detail(num_c);
            });
            if *num != my {
                let b = gtk::Button::with_label("Mensaje");
                let a = self.clone();
                let p = popover.clone();
                b.connect_clicked(move |_| {
                    p.popdown();
                    a.open_dm(num_c);
                });
                row.append(&b);
            }
            if st.saved.tracks.get(num).map(|t| t.len() > 1).unwrap_or(false) {
                let b = gtk::Button::with_label("Rastro");
                let a = self.clone();
                let p = popover.clone();
                b.connect_clicked(move |_| {
                    p.popdown();
                    a.show_track(num_c);
                });
                row.append(&b);
            }
            pop.append(&row);

            let mb = gtk::MenuButton::builder()
                .child(&gtk::Label::new(Some(label.as_str())))
                .popover(&popover)
                .tooltip_text(n.display_name())
                .build();
            mb.add_css_class("mapmarker");
            if *num == my {
                mb.add_css_class("me");
            } else if *fav {
                mb.add_css_class("fav");
            }
            if *old {
                mb.add_css_class("old");
            }
            let marker = shumate::Marker::builder().child(&mb).build();
            marker.set_location(*la, *lo);
            self.w.markers.add_marker(&marker);
        }

        if !self.map_centered.get() && !items.is_empty() {
            // Mi nodo; si no tiene posición, el más cercano por radio (menos saltos, sin MQTT).
            let (la, lo, z) = match items.iter().find(|i| i.0 == my) {
                Some(i) => (i.1, i.2, 10.0),
                None => {
                    let near = items
                        .iter()
                        .min_by_key(|i| {
                            let n = &st.nodes[&i.0];
                            (n.via_mqtt, n.hops_away.unwrap_or(99))
                        })
                        .unwrap_or(&items[0]);
                    (near.1, near.2, 9.0)
                }
            };
            // Sin animación: el mapa puede no estar visible todavía.
            if let Some(m) = self.w.map.map() {
                m.center_on(la, lo);
            }
            if let Some(v) = self.w.map.viewport() {
                v.set_zoom_level(z);
            }
            self.map_centered.set(true);
        }
    }
}

/// Oscurece la capa base del mapa (invierte los colores de las teselas).
/// Los marcadores y líneas son otras capas y no cambian.
pub(super) fn set_night(map: &shumate::SimpleMap, on: bool) {
    let Some(m) = map.map() else { return };
    if let Some(base) = m.first_child() {
        if on {
            base.add_css_class("noche");
        } else {
            base.remove_css_class("noche");
        }
    }
}
