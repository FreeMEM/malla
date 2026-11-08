//! Registro de paquetes recibidos, para ver qué pasa en la malla.

use super::*;

pub(super) struct LogUi {
    pub root: gtk::Box,
    pub list: gtk::ListBox,
    pub pause: gtk::ToggleButton,
    pub filter: gtk::DropDown,
}

const FILTERS: [&str; 9] = ["Todos", "Mensajes", "Posición", "Info de nodo", "Telemetría", "Enrutado", "Traceroute", "Cifrados", "Otros"];

fn matches(f: u32, p: &PacketLog) -> bool {
    let port = p.port.as_str();
    match f {
        0 => true,
        1 => port == "TEXT_MESSAGE_APP",
        2 => port == "POSITION_APP",
        3 => port == "NODEINFO_APP",
        4 => port == "TELEMETRY_APP",
        5 => port == "ROUTING_APP",
        6 => port == "TRACEROUTE_APP",
        7 => p.encrypted,
        _ => !p.encrypted
            && !["TEXT_MESSAGE_APP", "POSITION_APP", "NODEINFO_APP", "TELEMETRY_APP", "ROUTING_APP", "TRACEROUTE_APP"].contains(&port),
    }
}

pub(super) fn build_page() -> LogUi {
    let filter = gtk::DropDown::from_strings(&FILTERS);
    let pause = gtk::ToggleButton::builder().icon_name("media-playback-pause-symbolic").tooltip_text("Pausar").build();
    let clear = gtk::Button::builder().icon_name("edit-clear-all-symbolic").tooltip_text("Vaciar").name("vaciar").build();
    let info = gtk::Label::builder().label("Paquetes que oye tu nodo, el más reciente arriba.").xalign(0.0).hexpand(true).build();
    info.add_css_class("dim-label");
    let top = gtk::Box::builder().spacing(8).margin_start(12).margin_end(12).margin_top(10).margin_bottom(6).build();
    top.append(&info);
    top.append(&filter);
    top.append(&pause);
    top.append(&clear);
    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
    list.add_css_class("boxed-list");
    let scroll = gtk::ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(12)
        .build();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&top);
    root.append(&scroll);
    LogUi { root, list, pause, filter }
}

pub(super) fn connect_signals(app: &Rc<App>) {
    let a = app.clone();
    app.w.log_filter.connect_selected_notify(move |_| a.rebuild_log());
    let a = app.clone();
    app.w.log_pause.connect_toggled(move |b| {
        if !b.is_active() {
            a.rebuild_log();
        }
    });
    // Botón de vaciar: hermano siguiente del de pausa.
    if let Some(clear) = app.w.log_pause.next_sibling().and_downcast::<gtk::Button>() {
        let a = app.clone();
        clear.connect_clicked(move |_| {
            a.st.borrow_mut().packets.clear();
            a.rebuild_log();
        });
    }
}

impl App {
    pub(super) fn on_packet(&self, p: PacketLog) {
        let show = !self.w.log_pause.is_active() && matches(self.w.log_filter.selected(), &p);
        {
            let mut st = self.st.borrow_mut();
            st.packets.push_back(p.clone());
            while st.packets.len() > MAX_LOG {
                st.packets.pop_front();
            }
        }
        if show {
            let row = self.log_row(&p);
            self.w.log_list.prepend(&row);
            if let Some(last) = self.w.log_list.row_at_index(MAX_LOG as i32) {
                self.w.log_list.remove(&last);
            }
        }
    }

    fn log_row(&self, p: &PacketLog) -> gtk::ListBoxRow {
        let st = self.st.borrow();
        let to = if p.to == BROADCAST { "todos".to_string() } else { st.short_of(p.to) };
        let time = glib::DateTime::from_unix_local(p.time as i64)
            .and_then(|d| d.format("%H:%M:%S"))
            .map(|s| s.to_string())
            .unwrap_or_default();
        let mut extra = vec![];
        if let Some(s) = p.snr {
            extra.push(format!("SNR {s:.1}"));
        }
        if let Some(r) = p.rssi {
            extra.push(format!("RSSI {r}"));
        }
        if let Some(h) = p.hops {
            extra.push(hops_text(h));
        }
        if p.via_mqtt {
            extra.push("MQTT".into());
        }
        let line = format!(
            "{time}  {:>6} → {:<6}  {:<16} canal {}  {}  {}{}",
            st.short_of(p.from),
            to,
            p.port.trim_end_matches("_APP"),
            p.channel,
            extra.join(" · "),
            if p.size > 0 { format!("{} B  ", p.size) } else { String::new() },
            p.summary
        );
        let l = gtk::Label::builder().label(line).xalign(0.0).selectable(true).margin_start(8).margin_end(8).margin_top(3).margin_bottom(3).build();
        l.add_css_class("mono");
        if p.encrypted {
            l.add_css_class("dim-label");
        }
        gtk::ListBoxRow::builder().child(&l).activatable(false).build()
    }

    pub(super) fn rebuild_log(&self) {
        self.w.log_list.remove_all();
        let f = self.w.log_filter.selected();
        let pkts: Vec<PacketLog> = self.st.borrow().packets.iter().filter(|p| matches(f, p)).cloned().collect();
        for p in pkts.iter().rev() {
            let row = self.log_row(p);
            self.w.log_list.append(&row);
        }
    }
}
