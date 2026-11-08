//! Interfaz GTK4 + libadwaita. Secciones como el cliente web oficial:
//! Mensajes (canales y chats directos), Nodos y Mi nodo, más la ventana de conexión.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use shumate::prelude::*;
use tokio::sync::mpsc::UnboundedSender;

use crate::model::*;
use crate::store::{self, Saved};
use crate::tray;

/// Límite de texto de un mensaje, igual que el cliente web.
const MAX_BYTES: usize = 200;

const CSS: &str = r#"
.bubble { padding: 6px 10px; border-radius: 12px; background: alpha(@view_fg_color, 0.07); }
.bubble.mine { background: alpha(@accent_bg_color, 0.28); }
.meta { font-size: smaller; opacity: 0.7; }
.unread { background: @accent_bg_color; color: @accent_fg_color; border-radius: 10px;
          padding: 0 7px; font-weight: bold; font-size: smaller; }
.failed { color: @error_color; }
.convtitle { font-weight: bold; font-size: larger; padding: 10px 14px; }
menubutton.mapmarker > button.toggle { background: #1c71d8; color: white; border-radius: 12px;
             padding: 1px 8px; min-height: 0; min-width: 0; font-weight: bold; font-size: smaller;
             box-shadow: 0 1px 3px alpha(black, 0.5); }
menubutton.mapmarker.me > button.toggle { background: #26a269; color: white; }
.mapmarker.old { opacity: 0.55; }
"#;

#[derive(Default)]
struct State {
    my: u32,
    connected: bool,
    connecting: bool,
    target: Option<Target>,
    nodes: BTreeMap<u32, Node>,
    channels: BTreeMap<u32, ChannelInfo>,
    saved: Saved,
    current: Option<Conv>,
    unread: HashMap<Conv, u32>,
    open_dms: BTreeSet<u32>,
    expanded: HashSet<u32>,
    filter: String,
}

impl State {
    fn name_of(&self, num: u32) -> String {
        if num == BROADCAST {
            return "Todos".into();
        }
        if let Some(n) = self.nodes.get(&num) {
            if !n.long_name.is_empty() || !n.short_name.is_empty() {
                return n.display_name();
            }
        }
        self.saved
            .names
            .get(&num)
            .cloned()
            .unwrap_or_else(|| Node::id_str(num))
    }

    fn conv_title(&self, c: Conv) -> String {
        match c {
            Conv::Channel(i) => match self.channels.get(&i) {
                Some(ch) if !ch.name.is_empty() => format!("# {}", ch.name),
                Some(ch) if ch.role == 1 => "# Principal".into(),
                _ => format!("# Canal {i}"),
            },
            Conv::Direct(n) => self.name_of(n),
        }
    }

    /// Canales conocidos (del nodo o del historial) y chats directos, en orden.
    fn convs(&self) -> Vec<Conv> {
        let mut chans: BTreeSet<u32> = self.channels.keys().copied().collect();
        let mut dms: HashMap<u32, u32> = self.open_dms.iter().map(|n| (*n, 0)).collect();
        for m in &self.saved.messages {
            match m.conv {
                Conv::Channel(i) => {
                    if self.channels.is_empty() {
                        chans.insert(i);
                    }
                }
                Conv::Direct(n) => {
                    let t = dms.entry(n).or_insert(0);
                    *t = (*t).max(m.time);
                }
            }
        }
        if chans.is_empty() {
            chans.insert(0);
        }
        let mut dms: Vec<(u32, u32)> = dms.into_iter().collect();
        dms.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        chans
            .into_iter()
            .map(Conv::Channel)
            .chain(dms.into_iter().map(|(n, _)| Conv::Direct(n)))
            .collect()
    }
}

struct Widgets {
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    banner: adw::Banner,
    stack: adw::ViewStack,
    chats_page: adw::ViewStackPage,
    connect_btn: gtk::Button,
    spinner: gtk::Spinner,
    conv_list: gtk::ListBox,
    conv_title: gtk::Label,
    msg_list: gtk::ListBox,
    msg_scroll: gtk::ScrolledWindow,
    entry: gtk::Entry,
    send_btn: gtk::Button,
    counter: gtk::Label,
    nodes_list: gtk::ListBox,
    nodes_count: gtk::Label,
    me_box: gtk::Box,
    map: shumate::SimpleMap,
    markers: shumate::MarkerLayer,
}

/// Partes de la ventana de conexión que cambian con los eventos.
struct DialogParts {
    dialog: adw::Dialog,
    ble_group: adw::PreferencesGroup,
    ble_spinner: gtk::Spinner,
    ble_rows: Vec<adw::ActionRow>,
}

pub struct App {
    st: RefCell<State>,
    w: Widgets,
    cmd: UnboundedSender<Command>,
    dialog: RefCell<Option<DialogParts>>,
    rebuilding: Cell<bool>,
    nodes_dirty: Cell<bool>,
    map_centered: Cell<bool>,
    map_sig: RefCell<String>,
    tray: RefCell<Option<UnboundedSender<tray::Status>>>,
    tray_ok: Cell<bool>,
    tray_last: Cell<Option<tray::Status>>,
    gapp: adw::Application,
}

pub fn build(gapp: &adw::Application, cmd: UnboundedSender<Command>, ev: async_channel::Receiver<Event>) {
    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    if let Some(d) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&d, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    let mut st = State { saved: store::load(), ..Default::default() };
    st.current = st.convs().first().copied();

    // --- cabecera ---
    let stack = adw::ViewStack::new();
    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&switcher));
    let connect_btn = gtk::Button::with_label("Conectar");
    connect_btn.add_css_class("suggested-action");
    let spinner = gtk::Spinner::builder().spinning(true).build();
    spinner.set_visible(false);
    header.pack_start(&connect_btn);
    header.pack_start(&spinner);

    let banner = adw::Banner::builder()
        .title("Sin conexión con el nodo")
        .button_label("Conectar")
        .revealed(true)
        .build();

    // --- Mensajes ---
    let conv_list = gtk::ListBox::new();
    conv_list.add_css_class("navigation-sidebar");
    let conv_scroll = gtk::ScrolledWindow::builder()
        .child(&conv_list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .width_request(220)
        .build();

    let conv_title = gtk::Label::builder().xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
    conv_title.add_css_class("convtitle");
    let msg_list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
    msg_list.add_css_class("background");
    let msg_clamp = adw::Clamp::builder().maximum_size(820).child(&msg_list).build();
    let msg_scroll = gtk::ScrolledWindow::builder()
        .child(&msg_clamp)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let entry = gtk::Entry::builder()
        .hexpand(true)
        .placeholder_text("Escribe un mensaje…")
        .build();
    let counter = gtk::Label::new(Some(&format!("0/{MAX_BYTES}")));
    counter.add_css_class("meta");
    let send_btn = gtk::Button::from_icon_name("mail-send-symbolic");
    send_btn.set_tooltip_text(Some("Enviar"));
    send_btn.add_css_class("suggested-action");
    let input = gtk::Box::builder()
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(12)
        .build();
    input.append(&entry);
    input.append(&counter);
    input.append(&send_btn);

    let chat = gtk::Box::new(gtk::Orientation::Vertical, 0);
    chat.append(&conv_title);
    chat.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    chat.append(&msg_scroll);
    chat.append(&input);

    let paned = gtk::Paned::builder()
        .start_child(&conv_scroll)
        .end_child(&chat)
        .position(240)
        .shrink_start_child(false)
        .resize_start_child(false)
        .build();
    let chats_page = stack.add_titled_with_icon(&paned, Some("mensajes"), "Mensajes", "mail-unread-symbolic");

    // --- Nodos ---
    let search = gtk::SearchEntry::builder().placeholder_text("Buscar nodo").hexpand(true).build();
    let nodes_count = gtk::Label::new(None);
    nodes_count.add_css_class("dim-label");
    let top = gtk::Box::builder().spacing(12).build();
    top.append(&search);
    top.append(&nodes_count);
    let nodes_list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
    nodes_list.add_css_class("boxed-list");
    let nodes_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .build();
    nodes_box.append(&top);
    nodes_box.append(&nodes_list);
    let nodes_scroll = gtk::ScrolledWindow::builder()
        .child(&adw::Clamp::builder().maximum_size(900).child(&nodes_box).build())
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    stack.add_titled_with_icon(&nodes_scroll, Some("nodos"), "Nodos", "network-workgroup-symbolic");

    // --- Mapa ---
    let map = shumate::SimpleMap::new();
    let registry = shumate::MapSourceRegistry::with_defaults();
    if let Some(src) = registry.by_id(shumate::MAP_SOURCE_OSM_MAPNIK) {
        map.set_map_source(Some(&src));
    }
    map.set_vexpand(true);
    let viewport = map.viewport().expect("el mapa no tiene viewport");
    let markers = shumate::MarkerLayer::new(&viewport);
    if let Some(m) = map.map() {
        m.add_layer(&markers);
        // Centro inicial: península ibérica, hasta que lleguen posiciones.
        m.center_on(40.0, -3.7);
    }
    viewport.set_zoom_level(5.0);
    stack.add_titled_with_icon(&map, Some("mapa"), "Mapa", "mark-location-symbolic");

    // --- Mi nodo ---
    let me_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(12)
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .build();
    let me_scroll = gtk::ScrolledWindow::builder()
        .child(&adw::Clamp::builder().maximum_size(700).child(&me_box).build())
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    stack.add_titled_with_icon(&me_scroll, Some("yo"), "Mi nodo", "computer-symbolic");

    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&header);
    tv.add_top_bar(&banner);
    tv.set_content(Some(&stack));
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&tv));

    let window = adw::ApplicationWindow::builder()
        .application(gapp)
        .title("Malla")
        .default_width(1050)
        .default_height(720)
        .content(&toasts)
        .build();

    let app = Rc::new(App {
        st: RefCell::new(st),
        w: Widgets {
            window,
            toasts,
            banner,
            stack,
            chats_page,
            connect_btn,
            spinner,
            conv_list,
            conv_title,
            msg_list,
            msg_scroll,
            entry,
            send_btn,
            counter,
            nodes_list,
            nodes_count,
            me_box,
            map,
            markers,
        },
        map_centered: Cell::new(false),
        map_sig: RefCell::new(String::new()),
        tray: RefCell::new(None),
        tray_ok: Cell::new(false),
        tray_last: Cell::new(None),
        cmd,
        dialog: RefCell::new(None),
        rebuilding: Cell::new(false),
        nodes_dirty: Cell::new(true),
        gapp: gapp.clone(),
    });

    // --- señales ---
    {
        let a = app.clone();
        app.w.connect_btn.connect_clicked(move |_| a.open_connect_dialog());
        let a = app.clone();
        app.w.banner.connect_button_clicked(move |_| a.open_connect_dialog());
        let a = app.clone();
        app.w.conv_list.connect_row_selected(move |_, row| {
            if a.rebuilding.get() {
                return;
            }
            let Some(row) = row else { return };
            let convs = a.st.borrow().convs();
            if let Some(c) = convs.get(row.index() as usize) {
                a.select_conv(*c);
            }
        });
        let a = app.clone();
        app.w.entry.connect_changed(move |_| a.update_input());
        let a = app.clone();
        app.w.entry.connect_activate(move |_| a.send());
        let a = app.clone();
        app.w.send_btn.connect_clicked(move |_| a.send());
        let a = app.clone();
        search.connect_search_changed(move |s| {
            a.st.borrow_mut().filter = s.text().to_lowercase();
            a.rebuild_nodes();
        });
        let a = app.clone();
        app.w.window.connect_is_active_notify(move |w| {
            if w.is_active() {
                let cur = a.st.borrow().current;
                if let Some(c) = cur {
                    if a.st.borrow_mut().unread.remove(&c).is_some() {
                        a.rebuild_convs();
                    }
                }
            }
        });
    }

    // Icono en la bandeja del sistema (no en la demo).
    if !(store::is_demo() && std::env::args().any(|a| a == "--pagina")) {
        app.start_tray();
    }
    {
        // Con bandeja, cerrar la ventana solo la esconde, como Telegram.
        let a = app.clone();
        app.w.window.connect_close_request(move |w| {
            if a.tray_ok.get() {
                w.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
    }

    // Eventos de la radio.
    {
        let a = app.clone();
        glib::spawn_future_local(async move {
            while let Ok(e) = ev.recv().await {
                a.on_event(e);
            }
        });
    }

    // Refresco periódico de la lista de nodos ("hace X min").
    {
        let a = app.clone();
        let mut ticks = 0u32;
        glib::timeout_add_seconds_local(2, move || {
            ticks += 1;
            if a.nodes_dirty.replace(false) || ticks % 15 == 0 {
                a.rebuild_nodes();
                a.rebuild_me();
                a.rebuild_map();
            }
            glib::ControlFlow::Continue
        });
    }

    app.rebuild_convs();
    app.rebuild_messages();
    app.rebuild_nodes();
    app.rebuild_me();
    app.update_input();
    app.w.window.present();

    // `--pagina <nombre>`: abre en esa pestaña (mensajes, nodos, mapa, yo).
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--pagina") {
        match args.get(i + 1).map(String::as_str) {
            Some("conectar") => app.open_connect_dialog(),
            Some(p) => app.w.stack.set_visible_child_name(p),
            None => {}
        }
    }

    // Reconecta con la última conexión usada.
    let last = app.st.borrow().saved.last_target.clone();
    if let Some(t) = last {
        app.send_cmd(Command::Connect(t));
    }
}

impl App {
    fn start_tray(self: &Rc<Self>) {
        let bytes = glib::Bytes::from_static(include_bytes!("../data/es.freemem.Malla-64.png"));
        let Ok(tex) = gdk::Texture::from_bytes(&bytes) else { return };
        let mut dl = gdk::TextureDownloader::new(&tex);
        dl.set_format(gdk::MemoryFormat::R8g8b8a8);
        let (data, stride) = dl.download_bytes();
        let (w, h) = (tex.width() as usize, tex.height() as usize);
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            rgba.extend_from_slice(&data[y * stride..y * stride + w * 4]);
        }
        let (tx, rx) = async_channel::unbounded();
        *self.tray.borrow_mut() = Some(tray::spawn(rgba, w as i32, tx));
        let a = self.clone();
        glib::spawn_future_local(async move {
            while let Ok(act) = rx.recv().await {
                a.on_tray(act);
            }
        });
    }

    fn on_tray(self: &Rc<Self>, act: tray::Action) {
        let win = &self.w.window;
        match act {
            tray::Action::Ready(ok) => {
                self.tray_ok.set(ok);
                self.tray_last.set(None);
                self.update_tray();
            }
            tray::Action::Toggle => {
                if win.is_visible() && win.is_active() {
                    win.set_visible(false);
                } else {
                    win.present();
                }
            }
            tray::Action::Show => win.present(),
            tray::Action::Connect => {
                win.present();
                self.open_connect_dialog();
            }
            tray::Action::Disconnect => self.send_cmd(Command::Disconnect),
            tray::Action::Quit => self.gapp.quit(),
        }
    }

    fn update_tray(&self) {
        let st = tray::Status {
            unread: self.st.borrow().unread.values().sum(),
            connected: self.st.borrow().connected,
        };
        if self.tray_last.get() == Some(st) {
            return;
        }
        if let Some(t) = self.tray.borrow().as_ref() {
            if t.send(st).is_ok() {
                self.tray_last.set(Some(st));
            }
        }
    }

    fn send_cmd(&self, c: Command) {
        let _ = self.cmd.send(c);
    }

    fn toast(&self, text: &str) {
        if store::is_demo() && std::env::args().any(|a| a == "--pagina") {
            return; // capturas limpias
        }
        self.w.toasts.add_toast(adw::Toast::new(text));
    }

    fn save(&self) {
        store::save(&mut self.st.borrow_mut().saved);
    }

    // ---------- eventos ----------

    fn on_event(self: &Rc<Self>, e: Event) {
        match e {
            Event::BleDevices(res) => self.show_ble_devices(res),
            Event::Connecting(t) => {
                {
                    let mut st = self.st.borrow_mut();
                    st.connecting = true;
                    st.connected = false;
                    st.nodes.clear();
                    st.channels.clear();
                    st.my = 0;
                }
                self.w.spinner.set_visible(true);
                self.w.banner.set_title(&format!("Conectando con {}…", t.label()));
                self.w.banner.set_button_label(None);
                self.w.banner.set_revealed(true);
                self.update_header();
            }
            Event::Connected(t) => {
                {
                    let mut st = self.st.borrow_mut();
                    st.connecting = false;
                    st.connected = true;
                    st.saved.last_target = Some(t.clone());
                    st.target = Some(t);
                }
                self.save();
                self.w.spinner.set_visible(false);
                self.w.banner.set_revealed(false);
                self.update_header();
                self.update_input();
                self.toast("Conectado. Recibiendo datos del nodo…");
            }
            Event::Disconnected(reason) => {
                {
                    let mut st = self.st.borrow_mut();
                    st.connecting = false;
                    st.connected = false;
                    st.target = None;
                }
                self.w.spinner.set_visible(false);
                let title = match &reason {
                    Some(r) => format!("Sin conexión: {r}"),
                    None => "Sin conexión con el nodo".into(),
                };
                self.w.banner.set_title(&glib::markup_escape_text(&title));
                self.w.banner.set_button_label(Some("Conectar"));
                self.w.banner.set_revealed(true);
                self.update_header();
                self.update_input();
                self.nodes_dirty.set(true);
            }
            Event::MyNode(n) => self.st.borrow_mut().my = n,
            Event::ConfigComplete => {
                let n = self.st.borrow().nodes.len();
                self.toast(&format!("Nodo listo: {n} nodos conocidos en la malla"));
                self.save();
                self.rebuild_convs();
                self.rebuild_messages();
                self.nodes_dirty.set(true);
            }
            Event::Node(n) => {
                {
                    let mut st = self.st.borrow_mut();
                    if !n.long_name.is_empty() {
                        st.saved.names.insert(n.num, n.long_name.clone());
                    }
                    st.nodes.insert(n.num, n);
                }
                self.nodes_dirty.set(true);
            }
            Event::NodePatch(num, p) => {
                self.apply_patch(num, p);
                self.nodes_dirty.set(true);
            }
            Event::Channel(c) => {
                let first = {
                    let mut st = self.st.borrow_mut();
                    let first = st.channels.is_empty();
                    st.channels.insert(c.index, c);
                    first
                };
                if first && self.st.borrow().current.is_none() {
                    self.st.borrow_mut().current = Some(Conv::Channel(0));
                }
                self.rebuild_convs();
                self.update_conv_title();
            }
            Event::Message(m) => self.on_message(m),
            Event::Delivery { id, ok, error } => {
                let conv = {
                    let mut st = self.st.borrow_mut();
                    let my = st.my;
                    let m = st
                        .saved
                        .messages
                        .iter_mut()
                        .rev()
                        .find(|m| m.id == id && m.from == my && m.delivery != Delivery::Incoming);
                    match m {
                        Some(m) => {
                            // Un fallo no tapa una confirmación ya recibida.
                            if ok {
                                m.delivery = Delivery::Acked;
                                m.error = None;
                            } else if m.delivery != Delivery::Acked {
                                m.delivery = Delivery::Failed;
                                m.error = error;
                            }
                            Some(m.conv)
                        }
                        None => None,
                    }
                };
                if let Some(c) = conv {
                    self.save();
                    if self.st.borrow().current == Some(c) {
                        self.rebuild_messages();
                    }
                }
            }
            Event::Error(e) => self.toast(&e),
        }
    }

    fn apply_patch(&self, num: u32, p: NodePatch) {
        let mut st = self.st.borrow_mut();
        let n = st.nodes.entry(num).or_insert_with(|| Node { num, ..Default::default() });
        if let Some(t) = p.heard_at {
            n.last_heard = n.last_heard.max(t);
        }
        if p.snr.is_some() {
            n.snr = p.snr;
        }
        if p.rssi.is_some() {
            n.rssi = p.rssi;
        }
        if p.hops.is_some() {
            n.hops_away = p.hops;
        }
        if let Some((l, s, hw, role, pk)) = p.user {
            n.long_name = l;
            n.short_name = s;
            n.hw_model = hw;
            n.role = role;
            n.has_pubkey = pk;
        }
        if let Some((la, lo, alt)) = p.position {
            n.lat = Some(la);
            n.lon = Some(lo);
            n.alt = alt;
        }
        if p.battery.is_some() {
            n.battery = p.battery;
        }
        if p.voltage.is_some() {
            n.voltage = p.voltage;
        }
        if p.channel_util.is_some() {
            n.channel_util = p.channel_util;
        }
        if p.air_util_tx.is_some() {
            n.air_util_tx = p.air_util_tx;
        }
        if p.uptime.is_some() {
            n.uptime = p.uptime;
        }
        if !n.long_name.is_empty() {
            let name = n.long_name.clone();
            st.saved.names.insert(num, name);
        }
    }

    fn on_message(self: &Rc<Self>, m: Msg) {
        let (is_current, incoming, title, sender) = {
            let mut st = self.st.borrow_mut();
            if m.id != 0 && st.saved.messages.iter().any(|x| x.id == m.id && x.from == m.from) {
                return; // repetido
            }
            let is_current = st.current == Some(m.conv);
            let incoming = m.delivery == Delivery::Incoming;
            let title = st.conv_title(m.conv);
            let sender = st.name_of(m.from);
            st.saved.messages.push(m.clone());
            if incoming && (!is_current || !self.w.window.is_active()) {
                *st.unread.entry(m.conv).or_insert(0) += 1;
            }
            (is_current, incoming, title, sender)
        };
        self.save();
        if is_current {
            self.append_message_row(&m);
            self.scroll_to_end();
        }
        self.rebuild_convs();
        let capturas = std::env::args().any(|a| a == "--pagina");
        if incoming && !self.w.window.is_active() && !(store::is_demo() && capturas) {
            let n = gio::Notification::new(&format!("{sender} · {title}"));
            n.set_body(Some(&m.text));
            self.gapp.send_notification(Some("malla-mensaje"), &n);
        }
    }

    // ---------- cabecera y entrada ----------

    fn update_header(&self) {
        self.update_tray();
        let st = self.st.borrow();
        let b = &self.w.connect_btn;
        if st.connected {
            b.set_label("Conectado");
            b.remove_css_class("suggested-action");
            b.set_tooltip_text(st.target.as_ref().map(|t| t.label()).as_deref());
        } else if st.connecting {
            b.set_label("Conectando…");
            b.remove_css_class("suggested-action");
        } else {
            b.set_label("Conectar");
            b.add_css_class("suggested-action");
            b.set_tooltip_text(None);
        }
    }

    fn update_input(&self) {
        let st = self.st.borrow();
        let len = self.w.entry.text().len();
        self.w.counter.set_text(&format!("{len}/{MAX_BYTES}"));
        if len > MAX_BYTES {
            self.w.counter.add_css_class("failed");
        } else {
            self.w.counter.remove_css_class("failed");
        }
        let can = st.connected && st.current.is_some();
        self.w.entry.set_sensitive(can);
        self.w.send_btn.set_sensitive(can && len > 0 && len <= MAX_BYTES);
        self.w.entry.set_placeholder_text(Some(if st.connected {
            "Escribe un mensaje…"
        } else {
            "Conéctate a un nodo para escribir"
        }));
    }

    fn send(&self) {
        let text = self.w.entry.text().trim().to_string();
        if text.is_empty() || text.len() > MAX_BYTES {
            return;
        }
        let Some(conv) = self.st.borrow().current else { return };
        if !self.st.borrow().connected {
            return;
        }
        self.send_cmd(Command::SendText { conv, text });
        self.w.entry.set_text("");
    }

    // ---------- conversaciones ----------

    fn select_conv(&self, c: Conv) {
        {
            let mut st = self.st.borrow_mut();
            st.current = Some(c);
            st.unread.remove(&c);
        }
        self.rebuild_convs();
        self.rebuild_messages();
        self.update_input();
    }

    pub fn open_dm(&self, num: u32) {
        self.st.borrow_mut().open_dms.insert(num);
        self.w.stack.set_visible_child_name("mensajes");
        self.select_conv(Conv::Direct(num));
        self.w.entry.grab_focus();
    }

    fn rebuild_convs(&self) {
        self.rebuilding.set(true);
        let list = &self.w.conv_list;
        list.remove_all();
        let st = self.st.borrow();
        let convs = st.convs();
        let mut total = 0;
        for c in &convs {
            let row_box = gtk::Box::builder()
                .spacing(8)
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(4)
                .margin_end(4)
                .build();
            let icon = gtk::Image::from_icon_name(match c {
                Conv::Channel(_) => "network-wireless-symbolic",
                Conv::Direct(_) => "avatar-default-symbolic",
            });
            let label = gtk::Label::builder()
                .label(st.conv_title(*c))
                .xalign(0.0)
                .hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            row_box.append(&icon);
            row_box.append(&label);
            if let Some(n) = st.unread.get(c).filter(|n| **n > 0) {
                total += n;
                let b = gtk::Label::new(Some(&n.to_string()));
                b.add_css_class("unread");
                b.set_valign(gtk::Align::Center);
                row_box.append(&b);
            }
            let row = gtk::ListBoxRow::builder().child(&row_box).build();
            list.append(&row);
            if st.current == Some(*c) {
                list.select_row(Some(&row));
            }
        }
        self.w.chats_page.set_badge_number(total);
        self.update_tray();
        self.w.chats_page.set_needs_attention(total > 0);
        drop(st);
        self.rebuilding.set(false);
        self.update_conv_title();
    }

    fn update_conv_title(&self) {
        let st = self.st.borrow();
        let t = match st.current {
            Some(Conv::Direct(n)) => {
                let extra = st
                    .nodes
                    .get(&n)
                    .map(|nd| if nd.has_pubkey { " · cifrado de extremo a extremo" } else { "" })
                    .unwrap_or("");
                format!("{} ({}){extra}", st.name_of(n), Node::id_str(n))
            }
            Some(c) => st.conv_title(c),
            None => String::new(),
        };
        self.w.conv_title.set_text(&t);
    }

    fn rebuild_messages(&self) {
        self.w.msg_list.remove_all();
        let msgs: Vec<Msg> = {
            let st = self.st.borrow();
            let Some(c) = st.current else { return };
            st.saved.messages.iter().filter(|m| m.conv == c).cloned().collect()
        };
        if msgs.is_empty() {
            let l = gtk::Label::builder()
                .label("Aún no hay mensajes en esta conversación.")
                .margin_top(40)
                .build();
            l.add_css_class("dim-label");
            self.w.msg_list.append(&gtk::ListBoxRow::builder().child(&l).activatable(false).build());
        }
        for m in &msgs {
            self.append_message_row(m);
        }
        self.scroll_to_end();
        self.update_conv_title();
    }

    fn append_message_row(&self, m: &Msg) {
        // Quita el aviso de "no hay mensajes" si es lo único que hay.
        if let Some(first) = self.w.msg_list.row_at_index(0) {
            if first.child().map(|c| c.is::<gtk::Label>()).unwrap_or(false) {
                self.w.msg_list.remove(&first);
            }
        }
        let st = self.st.borrow();
        let mine = m.delivery != Delivery::Incoming;

        let bubble = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .halign(if mine { gtk::Align::End } else { gtk::Align::Start })
            .margin_start(if mine { 80 } else { 12 })
            .margin_end(if mine { 12 } else { 80 })
            .margin_top(3)
            .margin_bottom(3)
            .build();
        bubble.add_css_class("bubble");
        if mine {
            bubble.add_css_class("mine");
        }

        let who = if mine { "Yo".to_string() } else { st.name_of(m.from) };
        let head = gtk::Label::builder()
            .use_markup(true)
            .label(format!(
                "<b>{}</b>  <span size='small'>{}</span>",
                glib::markup_escape_text(&who),
                fmt_time(m.time)
            ))
            .xalign(0.0)
            .build();
        let body = gtk::Label::builder()
            .label(&m.text)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .xalign(0.0)
            .selectable(true)
            .build();
        bubble.append(&head);
        bubble.append(&body);

        let meta = match m.delivery {
            Delivery::Incoming => {
                let mut parts = vec![];
                if let Some(h) = m.hops {
                    parts.push(match h {
                        0 => "directo".to_string(),
                        1 => "1 salto".to_string(),
                        _ => format!("{h} saltos"),
                    });
                }
                if let Some(s) = m.snr {
                    parts.push(format!("SNR {s:.1} dB"));
                }
                if let Some(r) = m.rssi {
                    parts.push(format!("RSSI {r} dBm"));
                }
                parts.join(" · ")
            }
            Delivery::Pending => "Enviando… (sin confirmar)".into(),
            Delivery::Acked => match m.conv {
                Conv::Direct(_) => "✓ Entregado".into(),
                Conv::Channel(_) => "✓ Retransmitido por la malla".into(),
            },
            Delivery::Failed => format!("✗ {}", m.error.clone().unwrap_or_else(|| "Error".into())),
        };
        if !meta.is_empty() {
            let l = gtk::Label::builder().label(meta).xalign(if mine { 1.0 } else { 0.0 }).build();
            l.add_css_class("meta");
            if m.delivery == Delivery::Failed {
                l.add_css_class("failed");
            }
            bubble.append(&l);
        }

        let row = gtk::ListBoxRow::builder().child(&bubble).activatable(false).build();
        self.w.msg_list.append(&row);
    }

    fn scroll_to_end(&self) {
        let sw = self.w.msg_scroll.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(60), move || {
            let a = sw.vadjustment();
            a.set_value(a.upper() - a.page_size());
        });
    }

    // ---------- nodos ----------

    fn rebuild_nodes(self: &Rc<Self>) {
        let list = &self.w.nodes_list;
        list.remove_all();
        let st = self.st.borrow();
        let my = st.my;
        let mut nodes: Vec<&Node> = st
            .nodes
            .values()
            .filter(|n| {
                st.filter.is_empty()
                    || n.long_name.to_lowercase().contains(&st.filter)
                    || n.short_name.to_lowercase().contains(&st.filter)
                    || Node::id_str(n.num).contains(&st.filter)
            })
            .collect();
        nodes.sort_by(|a, b| {
            (b.num == my)
                .cmp(&(a.num == my))
                .then(b.favorite.cmp(&a.favorite))
                .then(b.last_heard.cmp(&a.last_heard))
        });
        self.w.nodes_count.set_text(&format!("{} nodos", st.nodes.len()));

        if nodes.is_empty() {
            let r = adw::ActionRow::builder()
                .title(if st.connected {
                    "Sin nodos todavía"
                } else {
                    "Conéctate a tu nodo para ver la malla"
                })
                .build();
            list.append(&r);
        }

        for n in nodes {
            let mut sub = vec![];
            if !n.short_name.is_empty() {
                sub.push(n.short_name.clone());
            }
            sub.push(Node::id_str(n.num));
            if n.num == my {
                sub.push("este nodo".into());
            } else {
                sub.push(ago(n.last_heard));
                sub.push(if n.via_mqtt {
                    "por MQTT".into()
                } else {
                    match n.hops_away {
                        Some(0) => "directo".into(),
                        Some(1) => "1 salto".into(),
                        Some(h) => format!("{h} saltos"),
                        None => "saltos ?".into(),
                    }
                });
                if let Some(s) = n.snr {
                    sub.push(format!("SNR {s:.1} dB"));
                }
            }
            if let Some(b) = n.battery {
                sub.push(if b > 100 { "enchufado".into() } else { format!("batería {b}%") });
            }

            let row = adw::ExpanderRow::builder()
                .title(n.display_name())
                .subtitle(sub.join(" · "))
                .use_markup(false)
                .expanded(st.expanded.contains(&n.num))
                .build();

            let num = n.num;
            if let (Some(la), Some(lo)) = (n.lat, n.lon) {
                let b = gtk::Button::builder()
                    .icon_name("find-location-symbolic")
                    .tooltip_text("Ver en el mapa")
                    .valign(gtk::Align::Center)
                    .build();
                b.add_css_class("flat");
                let a = self.clone();
                b.connect_clicked(move |_| a.show_on_map(la, lo));
                row.add_suffix(&b);
            }
            if num != my {
                let b = gtk::Button::builder()
                    .icon_name("mail-message-new-symbolic")
                    .tooltip_text("Mensaje directo")
                    .valign(gtk::Align::Center)
                    .build();
                b.add_css_class("flat");
                let a = self.clone();
                b.connect_clicked(move |_| a.open_dm(num));
                row.add_suffix(&b);
            }

            let detail = |t: &str, v: String| {
                let r = adw::ActionRow::builder().title(t).subtitle(v).use_markup(false).build();
                r.add_css_class("property");
                row.add_row(&r);
            };
            if !n.hw_model.is_empty() {
                detail("Modelo", n.hw_model.clone());
            }
            if !n.role.is_empty() {
                detail("Función", n.role.clone());
            }
            detail("Último contacto", if n.last_heard > 0 { fmt_datetime(n.last_heard) } else { "nunca".into() });
            if let Some(r) = n.rssi {
                detail("RSSI", format!("{r} dBm"));
            }
            if let Some(v) = n.voltage {
                detail("Voltaje", format!("{v:.2} V"));
            }
            if let Some(u) = n.channel_util {
                detail("Uso del canal", format!("{u:.1} %"));
            }
            if let Some(u) = n.air_util_tx {
                detail("Tiempo de emisión", format!("{u:.1} %"));
            }
            if let Some(u) = n.uptime {
                detail("Encendido", fmt_duration(u));
            }
            if let (Some(la), Some(lo)) = (n.lat, n.lon) {
                let alt = n.alt.map(|a| format!(", {a} m")).unwrap_or_default();
                detail("Posición", format!("{la:.5}, {lo:.5}{alt}"));
            }
            detail(
                "Cifrado directo",
                if n.has_pubkey { "sí (tiene clave pública)".into() } else { "no conocido".into() },
            );

            let a = self.clone();
            row.connect_expanded_notify(move |r| {
                let mut st = a.st.borrow_mut();
                if r.is_expanded() {
                    st.expanded.insert(num);
                } else {
                    st.expanded.remove(&num);
                }
            });
            list.append(&row);
        }
    }

    // ---------- mapa ----------

    fn show_on_map(&self, la: f64, lo: f64) {
        self.w.stack.set_visible_child_name("mapa");
        self.map_centered.set(true);
        if let Some(m) = self.w.map.map() {
            m.go_to_full(la, lo, 14.0);
        }
    }

    fn rebuild_map(self: &Rc<Self>) {
        let st = self.st.borrow();
        let my = st.my;
        let t = now();
        let mut items = vec![];
        for n in st.nodes.values() {
            let (Some(la), Some(lo)) = (n.lat, n.lon) else { continue };
            let label = if !n.short_name.is_empty() {
                n.short_name.clone()
            } else {
                format!("{:04x}", n.num & 0xffff)
            };
            let old = n.num != my && t.saturating_sub(n.last_heard) > 2 * 3600;
            items.push((n.num, la, lo, label, old));
        }
        // Solo se redibuja si algo cambió, para no cerrar una ficha abierta.
        let sig = format!("{my}{items:?}");
        if *self.map_sig.borrow() == sig {
            return;
        }
        *self.map_sig.borrow_mut() = sig;

        self.w.markers.remove_all();
        for (num, la, lo, label, old) in &items {
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
                match n.hops_away {
                    Some(0) => lines.push("directo".into()),
                    Some(1) => lines.push("1 salto".into()),
                    Some(h) => lines.push(format!("{h} saltos")),
                    None => {}
                }
            }
            if !n.hw_model.is_empty() {
                lines.push(n.hw_model.clone());
            }
            if let Some(b) = n.battery {
                lines.push(if b > 100 { "enchufado".into() } else { format!("batería {b}%") });
            }
            let info = gtk::Label::builder().label(lines.join("\n")).xalign(0.0).build();
            info.add_css_class("meta");
            pop.append(&info);
            let popover = gtk::Popover::builder().child(&pop).build();
            if *num != my {
                let b = gtk::Button::with_label("Mensaje directo");
                let a = self.clone();
                let num = *num;
                let p = popover.clone();
                b.connect_clicked(move |_| {
                    p.popdown();
                    a.open_dm(num);
                });
                pop.append(&b);
            }

            let mb = gtk::MenuButton::builder()
                .child(&gtk::Label::new(Some(label.as_str())))
                .popover(&popover)
                .tooltip_text(n.display_name())
                .build();
            mb.add_css_class("mapmarker");
            if *num == my {
                mb.add_css_class("me");
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

    // ---------- mi nodo ----------

    fn rebuild_me(self: &Rc<Self>) {
        let b = &self.w.me_box;
        while let Some(c) = b.first_child() {
            b.remove(&c);
        }
        let st = self.st.borrow();

        let conn = adw::PreferencesGroup::builder().title("Conexión").build();
        let row = adw::ActionRow::builder()
            .title(if st.connected { "Conectado" } else if st.connecting { "Conectando…" } else { "Desconectado" })
            .subtitle(st.target.as_ref().map(|t| t.label()).unwrap_or_default())
            .use_markup(false)
            .build();
        let btn = gtk::Button::builder()
            .label(if st.connected { "Desconectar" } else { "Conectar…" })
            .valign(gtk::Align::Center)
            .build();
        if st.connected {
            btn.add_css_class("destructive-action");
        }
        let a = self.clone();
        let connected = st.connected;
        btn.connect_clicked(move |_| {
            if connected {
                a.send_cmd(Command::Disconnect);
            } else {
                a.open_connect_dialog();
            }
        });
        row.add_suffix(&btn);
        conn.add(&row);
        b.append(&conn);

        if let Some(n) = st.nodes.get(&st.my) {
            let g = adw::PreferencesGroup::builder().title("Este nodo").build();
            let add = |t: &str, v: String| {
                let r = adw::ActionRow::builder().title(t).subtitle(v).use_markup(false).build();
                r.add_css_class("property");
                g.add(&r);
            };
            add("Nombre", n.display_name());
            add("Nombre corto", n.short_name.clone());
            add("Identificador", Node::id_str(n.num));
            add("Modelo", n.hw_model.clone());
            if !n.role.is_empty() {
                add("Función", n.role.clone());
            }
            if let Some(bt) = n.battery {
                add("Batería", if bt > 100 { "enchufado".into() } else { format!("{bt} %") });
            }
            if let Some(u) = n.channel_util {
                add("Uso del canal", format!("{u:.1} %"));
            }
            b.append(&g);
        }

        if !st.channels.is_empty() {
            let g = adw::PreferencesGroup::builder()
                .title("Canales")
                .description("Los canales y su cifrado se configuran en el nodo (de momento, desde la app del móvil).")
                .build();
            for c in st.channels.values() {
                let name = if c.name.is_empty() {
                    if c.role == 1 { "Principal (nombre por defecto)".to_string() } else { format!("Canal {}", c.index) }
                } else {
                    c.name.clone()
                };
                let r = adw::ActionRow::builder()
                    .title(name)
                    .subtitle(format!(
                        "Índice {} · {}",
                        c.index,
                        if c.role == 1 { "primario" } else { "secundario" }
                    ))
                    .use_markup(false)
                    .build();
                g.add(&r);
            }
            b.append(&g);
        }
    }

    // ---------- ventana de conexión ----------

    fn open_connect_dialog(self: &Rc<Self>) {
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

        if let Some(t) = st.saved.last_target.clone() {
            let g = adw::PreferencesGroup::builder().title("Última conexión").build();
            let r = adw::ActionRow::builder().title(t.label()).use_markup(false).build();
            r.add_suffix(&self.connect_button(t));
            g.add(&r);
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

    fn connect_button(self: &Rc<Self>, t: Target) -> gtk::Button {
        let b = gtk::Button::builder().label("Conectar").valign(gtk::Align::Center).build();
        b.add_css_class("suggested-action");
        let a = self.clone();
        b.connect_clicked(move |_| a.connect_to(t.clone()));
        b
    }

    fn connect_to(&self, t: Target) {
        self.send_cmd(Command::Connect(t));
        self.close_dialog();
    }

    fn close_dialog(&self) {
        let d = self.dialog.borrow().as_ref().map(|p| p.dialog.clone());
        if let Some(d) = d {
            d.close();
        }
    }

    fn show_ble_devices(self: &Rc<Self>, res: Result<Vec<(String, String)>, String>) {
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

// ---------- formato ----------

fn fmt_time(ts: u32) -> String {
    let Ok(dt) = glib::DateTime::from_unix_local(ts as i64) else { return String::new() };
    let today = glib::DateTime::now_local().ok().map(|n| n.ymd());
    let fmt = if Some(dt.ymd()) == today { "%H:%M" } else { "%d/%m %H:%M" };
    dt.format(fmt).map(|s| s.to_string()).unwrap_or_default()
}

fn fmt_datetime(ts: u32) -> String {
    glib::DateTime::from_unix_local(ts as i64)
        .and_then(|d| d.format("%d/%m/%Y %H:%M"))
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn ago(ts: u32) -> String {
    if ts == 0 {
        return "nunca oído".into();
    }
    let d = now().saturating_sub(ts);
    match d {
        0..=59 => "ahora".into(),
        60..=3599 => format!("hace {} min", d / 60),
        3600..=86399 => format!("hace {} h", d / 3600),
        _ => format!("hace {} días", d / 86400),
    }
}

fn fmt_duration(s: u32) -> String {
    let (d, h, m) = (s / 86400, (s % 86400) / 3600, (s % 3600) / 60);
    if d > 0 {
        format!("{d} d {h} h")
    } else if h > 0 {
        format!("{h} h {m} min")
    } else {
        format!("{m} min")
    }
}
