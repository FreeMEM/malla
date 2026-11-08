//! Interfaz GTK4 + libadwaita. Secciones como las apps oficiales:
//! Mensajes, Nodos, Mapa, Ajustes y Registro, más la ventana de conexión.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use meshtastic::protobufs;
use shumate::prelude::*;
use tokio::sync::mpsc::UnboundedSender;

use crate::model::*;
use crate::store::{self, Saved};
use crate::tray;

mod chart;
mod chat;
mod config;
mod connect;
mod format;
mod log;
mod map;
mod node_detail;
mod nodes;
mod settings;

use format::*;

/// Límite de texto de un mensaje, igual que las apps oficiales.
const MAX_BYTES: usize = 200;
const MAX_LOG: usize = 1000;

const CSS: &str = r#"
.bubble { padding: 6px 10px; border-radius: 12px; background: alpha(@view_fg_color, 0.07); }
.bubble.mine { background: alpha(#3584e4, 0.30); }
.meta { font-size: smaller; opacity: 0.7; }
.quote { border-left: 3px solid alpha(@view_fg_color, 0.4); padding-left: 6px; font-size: smaller; opacity: 0.8; }
.reaction { padding: 0 6px; min-height: 0; border-radius: 10px; font-size: smaller; }
.daysep { font-size: smaller; opacity: 0.6; margin: 8px 0 2px 0; }
.unread { background: #3584e4; color: white; border-radius: 10px;
          padding: 0 7px; font-weight: bold; font-size: smaller; }
.failed { color: @error_color; }
.convtitle { font-weight: bold; font-size: larger; }
.avatar { border-radius: 999px; min-width: 40px; min-height: 40px; font-weight: bold; font-size: smaller; color: white; }
.avatar.big { min-width: 64px; min-height: 64px; font-size: larger; }
.mono { font-family: monospace; font-size: smaller; }
menubutton.mapmarker > button.toggle { background: #1c71d8; color: white; border-radius: 12px;
             padding: 1px 8px; min-height: 0; min-width: 0; font-weight: bold; font-size: smaller;
             box-shadow: 0 1px 3px alpha(black, 0.5); }
menubutton.mapmarker.me > button.toggle { background: #26a269; }
menubutton.mapmarker.fav > button.toggle { background: #c64600; }
menubutton.mapmarker.wp > button.toggle { background: white; color: black; font-size: larger; padding: 0 4px; }
.mapmarker.old { opacity: 0.55; }
.mapbtn { box-shadow: 0 1px 3px alpha(black, 0.4); }
"#;

/// Orden de la lista de nodos.
#[derive(Clone, Copy, PartialEq, Default)]
enum NodeSort {
    #[default]
    LastHeard,
    Distance,
    Name,
    Snr,
    Hops,
}

#[derive(Default)]
struct NodeFilter {
    text: String,
    sort: NodeSort,
    only_active: bool,
    hide_mqtt: bool,
    only_fav: bool,
    show_ignored: bool,
}

#[derive(Default)]
struct State {
    my: u32,
    connected: bool,
    connecting: bool,
    target: Option<Target>,
    nodes: BTreeMap<u32, Node>,
    channels: BTreeMap<u32, ChannelInfo>,
    channels_full: BTreeMap<u32, protobufs::Channel>,
    /// Secciones de configuración por nombre ("device", "lora"…).
    config: BTreeMap<String, protobufs::Config>,
    modules: BTreeMap<String, protobufs::ModuleConfig>,
    metadata: Option<protobufs::DeviceMetadata>,
    my_user: Option<protobufs::User>,
    saved: Saved,
    current: Option<Conv>,
    unread: HashMap<Conv, u32>,
    open_dms: BTreeSet<u32>,
    nf: NodeFilter,
    packets: VecDeque<PacketLog>,
    reply_to: Option<u32>,
    /// Nodo cuyo rastro se dibuja en el mapa.
    track_of: Option<u32>,
}

impl State {
    fn name_of(&self, num: u32) -> String {
        if num == BROADCAST {
            return "Todos".into();
        }
        if num == self.my && num != 0 {
            if let Some(n) = self.nodes.get(&num) {
                return n.display_name();
            }
        }
        if let Some(n) = self.nodes.get(&num) {
            if !n.long_name.is_empty() || !n.short_name.is_empty() {
                return n.display_name();
            }
        }
        self.saved.names.get(&num).cloned().unwrap_or_else(|| Node::id_str(num))
    }

    fn short_of(&self, num: u32) -> String {
        self.nodes.get(&num).map(|n| n.short()).unwrap_or_else(|| format!("{:04x}", num & 0xffff))
    }

    fn conv_title(&self, c: Conv) -> String {
        match c {
            Conv::Channel(i) => format!("# {}", self.channel_name(i)),
            Conv::Direct(n) => self.name_of(n),
        }
    }

    fn channel_name(&self, i: u32) -> String {
        match self.channels.get(&i) {
            Some(ch) if !ch.name.is_empty() => ch.name.clone(),
            Some(ch) if ch.role == 1 => self.preset_name().unwrap_or_else(|| "Principal".into()),
            _ => format!("Canal {i}"),
        }
    }

    /// Nombre del preajuste LoRa (el canal primario sin nombre se llama así).
    fn preset_name(&self) -> Option<String> {
        let c = self.config.get("lora")?;
        if let Some(protobufs::config::PayloadVariant::Lora(l)) = &c.payload_variant {
            let p = protobufs::config::lo_ra_config::ModemPreset::try_from(l.modem_preset).ok()?;
            let name: String = p
                .as_str_name()
                .split('_')
                .map(|w| {
                    let mut c = w.chars();
                    c.next().map(|f| f.to_string() + &c.as_str().to_lowercase()).unwrap_or_default()
                })
                .collect();
            return Some(name);
        }
        None
    }

    fn my_pos(&self) -> Option<(f64, f64)> {
        let n = self.nodes.get(&self.my)?;
        Some((n.lat?, n.lon?))
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
        chans.into_iter().map(Conv::Channel).chain(dms.into_iter().map(|(n, _)| Conv::Direct(n))).collect()
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
    // Mensajes
    conv_list: gtk::ListBox,
    conv_title: gtk::Label,
    conv_sub: gtk::Label,
    mute_btn: gtk::ToggleButton,
    msg_list: gtk::ListBox,
    msg_scroll: gtk::ScrolledWindow,
    entry: gtk::Entry,
    send_btn: gtk::Button,
    counter: gtk::Label,
    reply_bar: gtk::Revealer,
    reply_label: gtk::Label,
    quick_box: gtk::Box,
    // Nodos
    nodes_list: gtk::ListBox,
    nodes_count: gtk::Label,
    // Mapa
    map: shumate::SimpleMap,
    markers: shumate::MarkerLayer,
    wp_markers: shumate::MarkerLayer,
    track_layer: RefCell<Option<shumate::PathLayer>>,
    clear_track: gtk::Button,
    // Ajustes
    settings_nav: adw::NavigationView,
    settings_root: gtk::Box,
    // Registro
    log_list: gtk::ListBox,
    log_pause: gtk::ToggleButton,
    log_filter: gtk::DropDown,
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
    settings_dirty: Cell<bool>,
    map_centered: Cell<bool>,
    map_sig: RefCell<String>,
    tray: RefCell<Option<UnboundedSender<tray::Status>>>,
    tray_ok: Cell<bool>,
    tray_last: Cell<Option<tray::Status>>,
    start_hidden: Cell<bool>,
    gapp: adw::Application,
}

fn args_have(flag: &str) -> bool {
    std::env::args().any(|a| a == flag)
}

fn arg_value(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1).cloned())
}

pub fn build(gapp: &adw::Application, cmd: UnboundedSender<Command>, ev: async_channel::Receiver<Event>) {
    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    if let Some(d) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&d, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    let mut st = State { saved: store::load(), ..Default::default() };
    st.current = st.convs().first().copied();
    apply_theme(&st.saved.prefs.theme);

    // --- cabecera ---
    let stack = adw::ViewStack::new();
    let switcher = adw::ViewSwitcher::builder().stack(&stack).policy(adw::ViewSwitcherPolicy::Wide).build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&switcher));
    let connect_btn = gtk::Button::with_label("Conectar");
    connect_btn.add_css_class("suggested-action");
    let spinner = gtk::Spinner::builder().spinning(true).visible(false).build();
    header.pack_start(&connect_btn);
    header.pack_start(&spinner);

    let menu = gio::Menu::new();
    menu.append(Some("Exportar mensajes (CSV)…"), Some("app.exportar"));
    menu.append(Some("Acerca de Malla"), Some("app.acerca"));
    menu.append(Some("Salir"), Some("app.salir"));
    let menu_btn = gtk::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&menu).build();
    header.pack_end(&menu_btn);

    let banner = adw::Banner::builder().title("Sin conexión con el nodo").button_label("Conectar").revealed(true).build();

    let chats = chat::build_page();
    let chats_page = stack.add_titled_with_icon(&chats.root, Some("mensajes"), "Mensajes", "mail-unread-symbolic");
    let nodes_ui = nodes::build_page();
    stack.add_titled_with_icon(&nodes_ui.root, Some("nodos"), "Nodos", "network-workgroup-symbolic");
    let map_ui = map::build_page();
    stack.add_titled_with_icon(&map_ui.root, Some("mapa"), "Mapa", "mark-location-symbolic");
    let settings_nav = adw::NavigationView::new();
    let settings_root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    settings_nav.add(&adw::NavigationPage::builder().title("Ajustes").tag("raiz").child(&settings_root).build());
    stack.add_titled_with_icon(&settings_nav, Some("ajustes"), "Ajustes", "emblem-system-symbolic");
    let log_ui = log::build_page();
    stack.add_titled_with_icon(&log_ui.root, Some("registro"), "Registro", "utilities-terminal-symbolic");

    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&header);
    tv.add_top_bar(&banner);
    tv.set_content(Some(&stack));
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&tv));

    let window = adw::ApplicationWindow::builder()
        .application(gapp)
        .title("Malla")
        .default_width(1100)
        .default_height(760)
        .content(&toasts)
        .build();

    let start_hidden = args_have("--oculta") || st.saved.prefs.start_hidden;
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
            conv_list: chats.conv_list,
            conv_title: chats.conv_title,
            conv_sub: chats.conv_sub,
            mute_btn: chats.mute_btn,
            msg_list: chats.msg_list,
            msg_scroll: chats.msg_scroll,
            entry: chats.entry,
            send_btn: chats.send_btn,
            counter: chats.counter,
            reply_bar: chats.reply_bar,
            reply_label: chats.reply_label,
            quick_box: chats.quick_box,
            nodes_list: nodes_ui.list,
            nodes_count: nodes_ui.count,
            map: map_ui.map,
            markers: map_ui.markers,
            wp_markers: map_ui.wp_markers,
            track_layer: RefCell::new(None),
            clear_track: map_ui.clear_track.clone(),
            settings_nav,
            settings_root,
            log_list: log_ui.list,
            log_pause: log_ui.pause,
            log_filter: log_ui.filter,
        },
        cmd,
        dialog: RefCell::new(None),
        rebuilding: Cell::new(false),
        nodes_dirty: Cell::new(true),
        settings_dirty: Cell::new(true),
        map_centered: Cell::new(false),
        map_sig: RefCell::new(String::new()),
        tray: RefCell::new(None),
        tray_ok: Cell::new(false),
        tray_last: Cell::new(None),
        start_hidden: Cell::new(start_hidden),
        gapp: gapp.clone(),
    });

    app.setup_actions();
    {
        let a = app.clone();
        app.w.connect_btn.connect_clicked(move |_| a.open_connect_dialog());
        let a = app.clone();
        app.w.banner.connect_button_clicked(move |_| a.banner_clicked());
        let a = app.clone();
        app.w.window.connect_is_active_notify(move |w| {
            if w.is_active() {
                let cur = a.st.borrow().current;
                let visible_chat = a.w.stack.visible_child_name().as_deref() == Some("mensajes");
                if let Some(c) = cur.filter(|_| visible_chat) {
                    if a.st.borrow_mut().unread.remove(&c).is_some() {
                        a.rebuild_convs();
                    }
                }
            }
        });
        let a = app.clone();
        app.w.stack.connect_visible_child_name_notify(move |s| {
            if s.visible_child_name().as_deref() == Some("mensajes") {
                let cur = a.st.borrow().current;
                if let Some(c) = cur {
                    if a.st.borrow_mut().unread.remove(&c).is_some() {
                        a.rebuild_convs();
                    }
                }
            }
        });
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
    chat::connect_signals(&app, &chats.extra);
    nodes::connect_signals(&app, &nodes_ui.extra);
    map::connect_signals(&app, &map_ui.extra);
    log::connect_signals(&app);

    // Icono en la bandeja (no en las capturas de la demo).
    if !(store::is_demo() && args_have("--pagina")) {
        app.start_tray();
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

    // Refresco periódico ("hace X min", listas que cambian a menudo).
    {
        let a = app.clone();
        let mut ticks = 0u32;
        glib::timeout_add_seconds_local(2, move || {
            ticks += 1;
            if a.nodes_dirty.replace(false) || ticks % 15 == 0 {
                a.rebuild_nodes();
                a.rebuild_map();
            }
            if a.settings_dirty.replace(false) {
                a.rebuild_settings();
            }
            glib::ControlFlow::Continue
        });
    }

    app.rebuild_convs();
    app.rebuild_messages();
    app.rebuild_quick();
    app.rebuild_nodes();
    app.rebuild_settings();
    app.rebuild_waypoints();
    app.update_input();
    if !app.start_hidden.get() {
        app.w.window.present();
    }

    // `--pagina`: mensajes, nodos, mapa, ajustes, registro, conectar,
    // ficha:<num hex>, ajustes:<sección>. Útil para capturas y pruebas.
    if let Some(p) = arg_value("--pagina") {
        let a = app.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
            if p == "conectar" {
                a.open_connect_dialog();
            } else if let Some(n) = p.strip_prefix("ficha:") {
                if let Ok(n) = u32::from_str_radix(n.trim_start_matches('!'), 16) {
                    a.w.stack.set_visible_child_name("nodos");
                    a.open_node_detail(n);
                }
            } else if let Some(k) = p.strip_prefix("ajustes:") {
                a.open_settings_section(k);
            } else if let Some(n) = p.strip_prefix("rastro:") {
                if let Ok(n) = u32::from_str_radix(n.trim_start_matches('!'), 16) {
                    a.show_track(n);
                }
            } else {
                a.w.stack.set_visible_child_name(&p);
            }
        });
    }

    // Reconecta con la última conexión usada.
    let last = app.st.borrow().saved.last_target.clone();
    if let Some(t) = last {
        app.send_cmd(Command::Connect(t));
    }
}

pub(super) fn apply_theme(theme: &str) {
    let sm = adw::StyleManager::default();
    sm.set_color_scheme(match theme {
        "claro" => adw::ColorScheme::ForceLight,
        "oscuro" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    });
}

/// Color estable para el avatar de un nodo.
pub(super) fn node_color(num: u32) -> String {
    const C: [&str; 10] = ["#1c71d8", "#26a269", "#c64600", "#813d9c", "#e5a50a", "#2190a4", "#c01c28", "#865e3c", "#3d3846", "#5e5c64"];
    C[(num.wrapping_mul(2654435761) >> 28) as usize % C.len()].to_string()
}

/// Círculo con el nombre corto del nodo, como en la app de Android.
pub(super) fn avatar(short: &str, num: u32, big: bool) -> gtk::Label {
    let l = gtk::Label::new(Some(short));
    l.add_css_class("avatar");
    if big {
        l.add_css_class("big");
    }
    l.set_valign(gtk::Align::Center);
    let p = gtk::CssProvider::new();
    p.load_from_string(&format!("label {{ background: {}; }}", node_color(num)));
    #[allow(deprecated)]
    l.style_context().add_provider(&p, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
    l
}

impl App {
    fn setup_actions(self: &Rc<Self>) {
        let a = self.clone();
        let act = gio::SimpleAction::new("exportar", None);
        act.connect_activate(move |_, _| a.export_csv());
        self.gapp.add_action(&act);

        let a = self.clone();
        let act = gio::SimpleAction::new("acerca", None);
        act.connect_activate(move |_, _| {
            let d = adw::AboutDialog::builder()
                .application_name("Malla")
                .application_icon("es.freemem.Malla")
                .developer_name("FreeMEM")
                .version(env!("CARGO_PKG_VERSION"))
                .website("https://github.com/FreeMEM/malla")
                .issue_url("https://github.com/FreeMEM/malla/issues")
                .license_type(gtk::License::Gpl30Only)
                .comments("Cliente de escritorio para la red LoRa Meshtastic.\nNo es un proyecto oficial de Meshtastic.")
                .build();
            d.present(Some(&a.w.window));
        });
        self.gapp.add_action(&act);

        let a = self.clone();
        let act = gio::SimpleAction::new("abrir-chat", Some(glib::VariantTy::STRING));
        act.connect_activate(move |_, v| {
            if let Some(c) = v.and_then(|v| v.str()).and_then(chat::conv_from_str) {
                a.open_conv(c);
            }
        });
        self.gapp.add_action(&act);

        let a = self.clone();
        let act = gio::SimpleAction::new("salir", None);
        act.connect_activate(move |_, _| a.gapp.quit());
        self.gapp.add_action(&act);
    }

    fn start_tray(self: &Rc<Self>) {
        let bytes = glib::Bytes::from_static(include_bytes!("../../data/es.freemem.Malla-64.png"));
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
                if !ok && self.start_hidden.get() {
                    win.present(); // sin bandeja no se puede arrancar escondida
                }
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

    fn admin(&self, list: Vec<protobufs::admin_message::PayloadVariant>) {
        if !self.st.borrow().connected {
            self.toast("No hay conexión con el nodo");
            return;
        }
        self.send_cmd(Command::Admin(list));
    }

    fn toast(&self, text: &str) {
        if store::is_demo() && args_have("--pagina") {
            return; // capturas limpias
        }
        self.w.toasts.add_toast(adw::Toast::new(&glib::markup_escape_text(text)));
    }

    fn save(&self) {
        store::save(&mut self.st.borrow_mut().saved);
    }

    fn banner_clicked(self: &Rc<Self>) {
        let target = self.st.borrow().target.clone();
        let reconnecting = self.w.banner.button_label().map(|l| l == "Reintentar ahora").unwrap_or(false);
        match target {
            Some(t) if reconnecting => self.send_cmd(Command::Connect(t)),
            _ => self.open_connect_dialog(),
        }
    }

    /// Pide confirmación antes de algo que no se deshace.
    fn confirm(self: &Rc<Self>, title: &str, body: &str, action: &str, f: impl Fn() + 'static) {
        let d = adw::AlertDialog::new(Some(title), Some(body));
        d.add_responses(&[("cancelar", "Cancelar"), ("ok", action)]);
        d.set_response_appearance("ok", adw::ResponseAppearance::Destructive);
        d.set_default_response(Some("cancelar"));
        d.set_close_response("cancelar");
        d.connect_response(None, move |_, r| {
            if r == "ok" {
                f();
            }
        });
        d.present(Some(&self.w.window));
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
                    st.channels_full.clear();
                    st.config.clear();
                    st.modules.clear();
                    st.metadata = None;
                    st.my = 0;
                    st.target = Some(t.clone());
                }
                self.w.spinner.set_visible(true);
                self.w.banner.set_title(&glib::markup_escape_text(&format!("Conectando con {}…", t.label())));
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
                self.settings_dirty.set(true);
            }
            Event::Reconnecting { target, secs, reason } => {
                {
                    let mut st = self.st.borrow_mut();
                    st.connecting = false;
                    st.connected = false;
                    st.target = Some(target);
                }
                self.w.spinner.set_visible(false);
                self.w.banner.set_title(&glib::markup_escape_text(&format!("{reason}. Reintento en {secs} s…")));
                self.w.banner.set_button_label(Some("Reintentar ahora"));
                self.w.banner.set_revealed(true);
                self.update_header();
                self.update_input();
            }
            Event::MyNode(n) => self.st.borrow_mut().my = n,
            Event::MyUser(u) => {
                self.st.borrow_mut().my_user = Some(u);
                self.settings_dirty.set(true);
            }
            Event::ConfigComplete => {
                let n = self.st.borrow().nodes.len();
                self.toast(&format!("Nodo listo: {n} nodos conocidos en la malla"));
                self.save();
                self.rebuild_convs();
                self.rebuild_messages();
                self.nodes_dirty.set(true);
                self.settings_dirty.set(true);
            }
            Event::Node(n) => {
                {
                    let mut st = self.st.borrow_mut();
                    if !n.long_name.is_empty() {
                        st.saved.names.insert(n.num, n.long_name.clone());
                    }
                    if let (Some(la), Some(lo)) = (n.lat, n.lon) {
                        let t = if n.last_heard > 0 { n.last_heard } else { now() };
                        st.saved.add_track(n.num, t, la, lo);
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
            }
            Event::ChannelFull(c) => {
                {
                    let mut st = self.st.borrow_mut();
                    if c.role == 0 {
                        st.channels.remove(&(c.index as u32));
                    }
                    st.channels_full.insert(c.index as u32, c);
                }
                self.settings_dirty.set(true);
            }
            Event::Config(c) => {
                if let Some(k) = config::variant_key(&c) {
                    self.st.borrow_mut().config.insert(k, c);
                    self.settings_dirty.set(true);
                }
            }
            Event::ModuleConfig(c) => {
                if let Some(k) = config::variant_key(&c) {
                    self.st.borrow_mut().modules.insert(k, c);
                    self.settings_dirty.set(true);
                }
            }
            Event::Metadata(m) => {
                self.st.borrow_mut().metadata = Some(m);
                self.settings_dirty.set(true);
            }
            Event::Message(m) => self.on_message(m),
            Event::Delivery { id, ok, error } => self.on_delivery(id, ok, error),
            Event::Trace(t) => self.on_trace(t),
            Event::Waypoint(w) => {
                {
                    let mut st = self.st.borrow_mut();
                    if w.expire != 0 && w.expire < now() {
                        st.saved.waypoints.remove(&w.id);
                    } else {
                        st.saved.waypoints.insert(w.id, w.clone());
                    }
                }
                self.save();
                self.rebuild_waypoints();
                if w.expire == 0 || w.expire >= now() {
                    let who = self.st.borrow().name_of(w.from);
                    self.toast(&format!("Nuevo punto en el mapa: {} {} (de {who})", w.icon_str(), w.name));
                }
            }
            Event::Packet(p) => self.on_packet(p),
            Event::Info(s) | Event::Error(s) => self.toast(&s),
        }
    }

    fn apply_patch(&self, num: u32, p: NodePatch) {
        let mut st = self.st.borrow_mut();
        let t = p.heard_at.unwrap_or_else(now);
        let n = st.nodes.entry(num).or_insert_with(|| Node { num, ..Default::default() });
        if let Some(h) = p.heard_at {
            n.last_heard = n.last_heard.max(h);
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
        if let Some(m) = p.via_mqtt {
            n.via_mqtt = m;
        }
        if let Some(u) = &p.user {
            n.long_name = u.long_name.clone();
            n.short_name = u.short_name.clone();
            n.hw_model = hw_name(u.hw_model);
            n.role = role_name(u.role);
            n.has_pubkey = !u.public_key.is_empty();
            n.licensed = u.is_licensed;
        }
        if let Some((la, lo, alt)) = p.position {
            n.lat = Some(la);
            n.lon = Some(lo);
            n.alt = alt;
        }
        macro_rules! set {
            ($($f:ident),*) => { $( if p.$f.is_some() { n.$f = p.$f; } )* };
        }
        set!(battery, voltage, channel_util, air_util_tx, uptime, temperature, humidity, pressure);
        let name = (!n.long_name.is_empty()).then(|| n.long_name.clone());
        if let Some(name) = name {
            st.saved.names.insert(num, name);
        }
        if let Some((la, lo, _)) = p.position {
            st.saved.add_track(num, t, la, lo);
        }
        if p.has_telemetry() {
            st.saved.add_sample(
                num,
                Sample {
                    t,
                    battery: p.battery,
                    voltage: p.voltage,
                    ch_util: p.channel_util,
                    air_tx: p.air_util_tx,
                    snr: p.snr,
                    temp: p.temperature,
                    hum: p.humidity,
                    press: p.pressure,
                },
            );
        }
    }

    fn on_delivery(self: &Rc<Self>, id: u32, ok: bool, error: Option<String>) {
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

    fn on_trace(self: &Rc<Self>, t: Trace) {
        self.st.borrow_mut().saved.add_trace(t.clone());
        self.save();
        let name = self.st.borrow().name_of(t.target);
        let toast = adw::Toast::builder()
            .title(glib::markup_escape_text(&format!("Traceroute a {name}: {} saltos", t.towards.len() + 1)))
            .button_label("Ver")
            .timeout(10)
            .build();
        let a = self.clone();
        toast.connect_button_clicked(move |_| a.show_trace(&t));
        self.w.toasts.add_toast(toast);
    }

    // ---------- cabecera ----------

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
        drop(st);
        self.settings_dirty.set(true);
    }

    fn export_csv(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder().title("Exportar mensajes").initial_name("malla-mensajes.csv").build();
        let a = self.clone();
        dialog.save(Some(&self.w.window), gio::Cancellable::NONE, move |res| {
            let Ok(file) = res else { return };
            let Some(path) = file.path() else { return };
            let st = a.st.borrow();
            let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
            let mut out = String::from("fecha,conversacion,de,id_de,texto,estado,snr,rssi,saltos\n");
            for m in st.saved.messages.iter().filter(|m| !m.emoji) {
                out += &format!(
                    "{},{},{},{},{},{:?},{},{},{}\n",
                    fmt_datetime(m.time),
                    q(&st.conv_title(m.conv)),
                    q(&st.name_of(m.from)),
                    Node::id_str(m.from),
                    q(&m.text),
                    m.delivery,
                    m.snr.map(|v| v.to_string()).unwrap_or_default(),
                    m.rssi.map(|v| v.to_string()).unwrap_or_default(),
                    m.hops.map(|v| v.to_string()).unwrap_or_default(),
                );
            }
            drop(st);
            match std::fs::write(&path, out) {
                Ok(()) => a.toast(&format!("Mensajes exportados a {}", path.display())),
                Err(e) => a.toast(&format!("No se pudo exportar: {e}")),
            }
        });
    }
}
