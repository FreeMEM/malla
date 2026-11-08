//! Conversaciones y mensajes: respuestas, reacciones, respuestas rápidas y silencio.

use super::*;

pub(super) struct ChatsUi {
    pub root: gtk::Paned,
    pub conv_list: gtk::ListBox,
    pub conv_title: gtk::Label,
    pub conv_sub: gtk::Label,
    pub mute_btn: gtk::ToggleButton,
    pub msg_list: gtk::ListBox,
    pub msg_scroll: gtk::ScrolledWindow,
    pub entry: gtk::Entry,
    pub send_btn: gtk::Button,
    pub counter: gtk::Label,
    pub reply_bar: gtk::Revealer,
    pub reply_label: gtk::Label,
    pub quick_box: gtk::Box,
    pub extra: ChatExtra,
}

pub(super) struct ChatExtra {
    reply_cancel: gtk::Button,
    emoji: gtk::EmojiChooser,
    quick_edit: gtk::Button,
    conv_menu: gtk::MenuButton,
}

const REACTIONS: [&str; 6] = ["👍", "❤️", "😂", "😮", "😢", "🙏"];

pub(super) fn build_page() -> ChatsUi {
    let conv_list = gtk::ListBox::new();
    conv_list.add_css_class("navigation-sidebar");
    let conv_scroll = gtk::ScrolledWindow::builder()
        .child(&conv_list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .width_request(240)
        .build();

    // Cabecera de la conversación.
    let conv_title = gtk::Label::builder().xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
    conv_title.add_css_class("convtitle");
    let conv_sub = gtk::Label::builder().xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
    conv_sub.add_css_class("meta");
    let titles = gtk::Box::builder().orientation(gtk::Orientation::Vertical).hexpand(true).build();
    titles.append(&conv_title);
    titles.append(&conv_sub);
    let mute_btn = gtk::ToggleButton::builder()
        .icon_name("preferences-system-notifications-symbolic")
        .tooltip_text("Silenciar avisos de esta conversación")
        .valign(gtk::Align::Center)
        .build();
    mute_btn.add_css_class("flat");
    let conv_menu = gtk::MenuButton::builder().icon_name("view-more-symbolic").valign(gtk::Align::Center).build();
    conv_menu.add_css_class("flat");
    let head = gtk::Box::builder().spacing(6).margin_start(14).margin_end(10).margin_top(8).margin_bottom(8).build();
    head.append(&titles);
    head.append(&mute_btn);
    head.append(&conv_menu);

    let msg_list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
    msg_list.add_css_class("background");
    let msg_clamp = adw::Clamp::builder().maximum_size(860).child(&msg_list).build();
    let msg_scroll = gtk::ScrolledWindow::builder()
        .child(&msg_clamp)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();

    // Barra "respondiendo a…".
    let reply_label = gtk::Label::builder().xalign(0.0).hexpand(true).ellipsize(gtk::pango::EllipsizeMode::End).build();
    reply_label.add_css_class("quote");
    let reply_cancel = gtk::Button::from_icon_name("window-close-symbolic");
    reply_cancel.add_css_class("flat");
    reply_cancel.set_tooltip_text(Some("No responder"));
    let reply_box = gtk::Box::builder().spacing(6).margin_start(12).margin_end(12).margin_top(6).build();
    reply_box.append(&reply_label);
    reply_box.append(&reply_cancel);
    let reply_bar = gtk::Revealer::builder().child(&reply_box).reveal_child(false).build();

    // Respuestas rápidas.
    let quick_box = gtk::Box::builder().spacing(6).build();
    let quick_edit = gtk::Button::builder().icon_name("document-edit-symbolic").tooltip_text("Editar respuestas rápidas").build();
    quick_edit.add_css_class("flat");
    let quick_row = gtk::Box::builder().spacing(6).margin_start(12).margin_end(12).margin_top(6).build();
    let quick_scroll = gtk::ScrolledWindow::builder()
        .child(&quick_box)
        .hexpand(true)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .build();
    quick_row.append(&quick_scroll);
    quick_row.append(&quick_edit);

    let entry = gtk::Entry::builder().hexpand(true).placeholder_text("Escribe un mensaje…").build();
    let counter = gtk::Label::new(Some(&format!("0/{MAX_BYTES}")));
    counter.add_css_class("meta");
    let emoji = gtk::EmojiChooser::new();
    let emoji_btn = gtk::MenuButton::builder().icon_name("face-smile-symbolic").popover(&emoji).tooltip_text("Emoji").build();
    let send_btn = gtk::Button::from_icon_name("mail-send-symbolic");
    send_btn.set_tooltip_text(Some("Enviar"));
    send_btn.add_css_class("suggested-action");
    let input = gtk::Box::builder().spacing(8).margin_start(12).margin_end(12).margin_top(6).margin_bottom(12).build();
    input.append(&emoji_btn);
    input.append(&entry);
    input.append(&counter);
    input.append(&send_btn);

    let chat = gtk::Box::new(gtk::Orientation::Vertical, 0);
    chat.append(&head);
    chat.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    chat.append(&msg_scroll);
    chat.append(&reply_bar);
    chat.append(&quick_row);
    chat.append(&input);

    let root = gtk::Paned::builder()
        .start_child(&conv_scroll)
        .end_child(&chat)
        .position(260)
        .shrink_start_child(false)
        .resize_start_child(false)
        .build();

    ChatsUi {
        root,
        conv_list,
        conv_title,
        conv_sub,
        mute_btn,
        msg_list,
        msg_scroll,
        entry,
        send_btn,
        counter,
        reply_bar,
        reply_label,
        quick_box,
        extra: ChatExtra { reply_cancel, emoji, quick_edit, conv_menu },
    }
}

pub(super) fn connect_signals(app: &Rc<App>, x: &ChatExtra) {
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
    x.reply_cancel.connect_clicked(move |_| a.set_reply(None));
    let a = app.clone();
    x.emoji.connect_emoji_picked(move |_, e| {
        let pos = a.w.entry.position();
        let mut p = pos;
        a.w.entry.insert_text(e, &mut p);
        a.w.entry.set_position(p);
        a.w.entry.grab_focus_without_selecting();
    });
    let a = app.clone();
    x.quick_edit.connect_clicked(move |_| a.open_settings_section("app"));
    let a = app.clone();
    app.w.mute_btn.connect_toggled(move |b| {
        let Some(c) = a.st.borrow().current else { return };
        {
            let mut st = a.st.borrow_mut();
            let muted = st.saved.is_muted(c);
            if b.is_active() && !muted {
                st.saved.muted.push(c);
            } else if !b.is_active() && muted {
                st.saved.muted.retain(|m| *m != c);
            }
        }
        a.save();
        a.update_conv_title();
        a.rebuild_convs();
    });

    // Menú de la conversación.
    let pop = gtk::Popover::new();
    let bx = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).build();
    let item = |label: &str| {
        let b = gtk::Button::builder().label(label).build();
        b.add_css_class("flat");
        b.child().and_downcast::<gtk::Label>().inspect(|l| l.set_xalign(0.0));
        bx.append(&b);
        b
    };
    let read = item("Marcar como leída");
    let node = item("Ver ficha del nodo");
    let clear = item("Borrar conversación de este equipo");
    pop.set_child(Some(&bx));
    x.conv_menu.set_popover(Some(&pop));
    let a = app.clone();
    let p = pop.clone();
    read.connect_clicked(move |_| {
        p.popdown();
        let c = a.st.borrow().current;
        if let Some(c) = c {
            a.st.borrow_mut().unread.remove(&c);
            a.rebuild_convs();
        }
    });
    let a = app.clone();
    let p = pop.clone();
    node.connect_clicked(move |_| {
        p.popdown();
        if let Some(Conv::Direct(n)) = a.st.borrow().current {
            a.open_node_detail(n);
        }
    });
    let a = app.clone();
    let p = pop.clone();
    clear.connect_clicked(move |_| {
        p.popdown();
        let Some(c) = a.st.borrow().current else { return };
        let title = a.st.borrow().conv_title(c);
        let a2 = a.clone();
        a.confirm(
            "¿Borrar la conversación?",
            &format!("Se borran de este equipo los mensajes de {title}. No afecta a la radio ni a nadie más."),
            "Borrar",
            move || {
                a2.st.borrow_mut().saved.messages.retain(|m| m.conv != c);
                a2.save();
                a2.rebuild_convs();
                a2.rebuild_messages();
            },
        );
    });
    let a = app.clone();
    pop.connect_show(move |_| {
        let is_dm = matches!(a.st.borrow().current, Some(Conv::Direct(_)));
        node.set_visible(is_dm);
    });
}

fn day_key(ts: u32) -> Option<(i32, i32, i32)> {
    glib::DateTime::from_unix_local(ts as i64).ok().map(|d| d.ymd())
}

fn day_label(ts: u32) -> String {
    let Ok(d) = glib::DateTime::from_unix_local(ts as i64) else { return String::new() };
    let today = glib::DateTime::now_local().ok();
    let yest = today.as_ref().and_then(|t| t.add_days(-1).ok());
    if today.map(|t| t.ymd()) == Some(d.ymd()) {
        "Hoy".into()
    } else if yest.map(|t| t.ymd()) == Some(d.ymd()) {
        "Ayer".into()
    } else {
        d.format("%A %e de %B").map(|s| s.to_string()).unwrap_or_default()
    }
}

impl App {
    pub(super) fn on_message(self: &Rc<Self>, m: Msg) {
        let (is_current, incoming, title, sender, notify) = {
            let mut st = self.st.borrow_mut();
            if m.id != 0 && st.saved.messages.iter().any(|x| x.id == m.id && x.from == m.from) {
                return; // repetido
            }
            let is_current = st.current == Some(m.conv);
            let incoming = m.delivery == Delivery::Incoming;
            let title = st.conv_title(m.conv);
            let sender = st.name_of(m.from);
            let ignored = st.nodes.get(&m.from).map(|n| n.ignored).unwrap_or(false);
            if ignored {
                return;
            }
            st.saved.messages.push(m.clone());
            let in_view = is_current && self.w.window.is_active() && self.w.stack.visible_child_name().as_deref() == Some("mensajes");
            if incoming && !in_view && !m.emoji {
                *st.unread.entry(m.conv).or_insert(0) += 1;
            }
            let prefs = &st.saved.prefs;
            let notify = incoming
                && !in_view
                && prefs.notify
                && (prefs.notify_channels || matches!(m.conv, Conv::Direct(_)))
                && !st.saved.is_muted(m.conv);
            (is_current, incoming, title, sender, notify)
        };
        self.save();
        if is_current {
            if m.emoji || m.reply_id.is_some() {
                self.rebuild_messages();
            } else {
                self.append_message_row(&m);
                self.scroll_to_end();
            }
        }
        self.rebuild_convs();
        let capturas = store::is_demo() && args_have("--pagina");
        if incoming && notify && !capturas {
            let body = if m.emoji { format!("Ha reaccionado con {}", m.text) } else { m.text.clone() };
            let n = gio::Notification::new(&format!("{sender} · {title}"));
            n.set_body(Some(&body));
            n.set_default_action_and_target_value("app.abrir-chat", Some(&conv_variant(m.conv)));
            self.gapp.send_notification(Some("malla-mensaje"), &n);
        }
    }

    pub(super) fn update_input(&self) {
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
        self.w.quick_box.set_sensitive(can);
        self.w.entry.set_placeholder_text(Some(if st.connected { "Escribe un mensaje…" } else { "Conéctate a un nodo para escribir" }));
    }

    pub(super) fn send(&self) {
        let text = self.w.entry.text().trim().to_string();
        if text.is_empty() || text.len() > MAX_BYTES {
            return;
        }
        let Some(conv) = self.st.borrow().current else { return };
        if !self.st.borrow().connected {
            return;
        }
        let reply_id = self.st.borrow().reply_to;
        self.send_cmd(Command::SendText { conv, text, reply_id, emoji: false });
        self.w.entry.set_text("");
        self.set_reply(None);
    }

    fn send_reaction(&self, m: &Msg, emoji: &str) {
        if !self.st.borrow().connected {
            self.toast("No hay conexión con el nodo");
            return;
        }
        self.send_cmd(Command::SendText { conv: m.conv, text: emoji.to_string(), reply_id: Some(m.id), emoji: true });
    }

    pub(super) fn set_reply(&self, id: Option<u32>) {
        self.st.borrow_mut().reply_to = id;
        match id {
            Some(id) => {
                let st = self.st.borrow();
                let txt = st
                    .saved
                    .messages
                    .iter()
                    .find(|m| m.id == id)
                    .map(|m| format!("Respondiendo a {}: {}", st.name_of(m.from), m.text))
                    .unwrap_or_else(|| "Respondiendo".into());
                self.w.reply_label.set_text(&txt);
                self.w.reply_bar.set_reveal_child(true);
                self.w.entry.grab_focus();
            }
            None => self.w.reply_bar.set_reveal_child(false),
        }
    }

    pub(super) fn rebuild_quick(self: &Rc<Self>) {
        let b = &self.w.quick_box;
        while let Some(c) = b.first_child() {
            b.remove(&c);
        }
        let quick = self.st.borrow().saved.prefs.quick.clone();
        for q in quick {
            let btn = gtk::Button::with_label(&q);
            btn.add_css_class("pill");
            btn.add_css_class("small");
            let a = self.clone();
            btn.connect_clicked(move |_| {
                a.w.entry.set_text(&q);
                a.w.entry.set_position(-1);
                a.w.entry.grab_focus_without_selecting();
            });
            b.append(&btn);
        }
    }

    pub(super) fn select_conv(self: &Rc<Self>, c: Conv) {
        {
            let mut st = self.st.borrow_mut();
            if st.current != Some(c) {
                st.reply_to = None;
            }
            st.current = Some(c);
            st.unread.remove(&c);
        }
        self.w.reply_bar.set_reveal_child(false);
        self.rebuild_convs();
        self.rebuild_messages();
        self.update_input();
    }

    pub(super) fn open_dm(self: &Rc<Self>, num: u32) {
        self.st.borrow_mut().open_dms.insert(num);
        self.w.stack.set_visible_child_name("mensajes");
        self.select_conv(Conv::Direct(num));
        self.w.window.present();
        self.w.entry.grab_focus();
    }

    pub(super) fn open_conv(self: &Rc<Self>, c: Conv) {
        if let Conv::Direct(n) = c {
            self.st.borrow_mut().open_dms.insert(n);
        }
        self.w.stack.set_visible_child_name("mensajes");
        self.select_conv(c);
        self.w.window.present();
    }

    pub(super) fn rebuild_convs(&self) {
        self.rebuilding.set(true);
        let list = &self.w.conv_list;
        list.remove_all();
        let st = self.st.borrow();
        let convs = st.convs();
        let mut total = 0;
        for c in &convs {
            let row_box = gtk::Box::builder().spacing(10).margin_top(4).margin_bottom(4).margin_start(2).margin_end(2).build();
            match c {
                Conv::Channel(_) => {
                    let i = gtk::Image::from_icon_name("network-wireless-symbolic");
                    i.set_pixel_size(20);
                    i.set_size_request(32, 32);
                    row_box.append(&i);
                }
                Conv::Direct(n) => {
                    let av = avatar(&st.short_of(*n), *n, false);
                    av.set_size_request(32, 32);
                    row_box.append(&av);
                }
            }
            let last = st.saved.messages.iter().rev().find(|m| m.conv == *c && !m.emoji);
            let texts = gtk::Box::builder().orientation(gtk::Orientation::Vertical).hexpand(true).valign(gtk::Align::Center).build();
            let label = gtk::Label::builder().label(st.conv_title(*c)).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
            texts.append(&label);
            if let Some(m) = last {
                let who = if m.from == st.my { "Tú".to_string() } else { st.short_of(m.from) };
                let p = gtk::Label::builder()
                    .label(format!("{who}: {}", m.text))
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();
                p.add_css_class("meta");
                texts.append(&p);
            }
            row_box.append(&texts);
            if st.saved.is_muted(*c) {
                let i = gtk::Image::from_icon_name("notifications-disabled-symbolic");
                i.add_css_class("dim-label");
                row_box.append(&i);
            }
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
        self.w.chats_page.set_needs_attention(total > 0);
        drop(st);
        self.rebuilding.set(false);
        self.update_tray();
        self.update_conv_title();
    }

    pub(super) fn update_conv_title(&self) {
        let st = self.st.borrow();
        let Some(c) = st.current else {
            self.w.conv_title.set_text("");
            self.w.conv_sub.set_text("");
            return;
        };
        self.w.conv_title.set_text(&st.conv_title(c));
        let sub = match c {
            Conv::Channel(i) => {
                let users = st.saved.messages.iter().filter(|m| m.conv == c).map(|m| m.from).collect::<BTreeSet<_>>().len();
                let role = match st.channels.get(&i).map(|c| c.role) {
                    Some(1) => "canal primario",
                    Some(_) => "canal secundario",
                    None => "canal",
                };
                format!("{role} {i} · {users} participantes en el historial")
            }
            Conv::Direct(n) => {
                let mut v = vec![Node::id_str(n)];
                if let Some(nd) = st.nodes.get(&n) {
                    v.push(ago(nd.last_heard));
                    if let Some(h) = nd.hops_away {
                        v.push(hops_text(h));
                    }
                    if nd.has_pubkey {
                        v.push("🔒 cifrado de extremo a extremo".into());
                    }
                }
                v.join(" · ")
            }
        };
        self.w.conv_sub.set_text(&sub);
        let muted = st.saved.is_muted(c);
        drop(st);
        if self.w.mute_btn.is_active() != muted {
            self.w.mute_btn.set_active(muted);
        }
        self.w.mute_btn.set_icon_name(if muted { "notifications-disabled-symbolic" } else { "preferences-system-notifications-symbolic" });
    }

    pub(super) fn rebuild_messages(self: &Rc<Self>) {
        self.w.msg_list.remove_all();
        let msgs: Vec<Msg> = {
            let st = self.st.borrow();
            let Some(c) = st.current else { return };
            st.saved.messages.iter().filter(|m| m.conv == c && !m.emoji).cloned().collect()
        };
        if msgs.is_empty() {
            let l = gtk::Label::builder().label("Aún no hay mensajes en esta conversación.").margin_top(40).build();
            l.add_css_class("dim-label");
            self.w.msg_list.append(&gtk::ListBoxRow::builder().child(&l).activatable(false).name("vacio").build());
        }
        let mut last_day = None;
        for m in &msgs {
            let d = day_key(m.time);
            if d != last_day {
                self.append_day(m.time);
                last_day = d;
            }
            self.append_bubble(m);
        }
        self.scroll_to_end();
        self.update_conv_title();
    }

    fn append_day(&self, ts: u32) {
        let l = gtk::Label::new(Some(&day_label(ts)));
        l.add_css_class("daysep");
        self.w.msg_list.append(&gtk::ListBoxRow::builder().child(&l).activatable(false).name(format!("dia{ts}")).build());
    }

    /// Añade un mensaje al final, con separador de día si cambia.
    pub(super) fn append_message_row(self: &Rc<Self>, m: &Msg) {
        if let Some(first) = self.w.msg_list.row_at_index(0) {
            if first.widget_name() == "vacio" {
                self.w.msg_list.remove(&first);
            }
        }
        let prev = {
            let st = self.st.borrow();
            st.saved.messages.iter().rev().filter(|x| x.conv == m.conv && !x.emoji && x.id != m.id).map(|x| x.time).next()
        };
        if prev.and_then(day_key) != day_key(m.time) {
            self.append_day(m.time);
        }
        self.append_bubble(m);
    }

    fn append_bubble(self: &Rc<Self>, m: &Msg) {
        let st = self.st.borrow();
        let mine = m.delivery != Delivery::Incoming;

        let bubble = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .halign(if mine { gtk::Align::End } else { gtk::Align::Start })
            .build();
        bubble.add_css_class("bubble");
        if mine {
            bubble.add_css_class("mine");
        }

        let who = if mine { "Tú".to_string() } else { st.name_of(m.from) };
        let head = gtk::Label::builder()
            .use_markup(true)
            .label(format!("<b>{}</b>  <span size='small'>{}</span>", glib::markup_escape_text(&who), fmt_time(m.time)))
            .xalign(0.0)
            .build();
        bubble.append(&head);

        if let Some(rid) = m.reply_id {
            let orig = st.saved.messages.iter().find(|x| x.id == rid && x.conv == m.conv);
            let q = match orig {
                Some(o) => format!("↪ {}: {}", if o.from == st.my { "Tú".to_string() } else { st.name_of(o.from) }, o.text),
                None => "↪ mensaje no disponible".into(),
            };
            let ql = gtk::Label::builder().label(q).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).max_width_chars(50).build();
            ql.add_css_class("quote");
            bubble.append(&ql);
        }

        let body = gtk::Label::builder()
            .label(&m.text)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .xalign(0.0)
            .selectable(true)
            .max_width_chars(60)
            .build();
        bubble.append(&body);

        let meta = match m.delivery {
            Delivery::Incoming => {
                let mut parts = vec![];
                if let Some(h) = m.hops {
                    parts.push(hops_text(h));
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

        // Reacciones agrupadas por emoji.
        let mut reacts: Vec<(String, Vec<String>)> = vec![];
        for r in st.saved.messages.iter().filter(|r| r.emoji && r.reply_id == Some(m.id) && r.conv == m.conv) {
            let name = if r.from == st.my { "Tú".to_string() } else { st.name_of(r.from) };
            match reacts.iter_mut().find(|(e, _)| *e == r.text) {
                Some((_, v)) => v.push(name),
                None => reacts.push((r.text.clone(), vec![name])),
            }
        }
        drop(st);

        let outer = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .halign(if mine { gtk::Align::End } else { gtk::Align::Start })
            .margin_start(if mine { 80 } else { 12 })
            .margin_end(if mine { 12 } else { 80 })
            .margin_top(3)
            .margin_bottom(3)
            .build();
        outer.append(&bubble);
        if !reacts.is_empty() {
            let rb = gtk::Box::builder().spacing(4).halign(if mine { gtk::Align::End } else { gtk::Align::Start }).build();
            for (e, names) in reacts {
                let b = gtk::Button::builder()
                    .label(if names.len() > 1 { format!("{e} {}", names.len()) } else { e.clone() })
                    .tooltip_text(names.join(", "))
                    .build();
                b.add_css_class("reaction");
                let a = self.clone();
                let mm = m.clone();
                b.connect_clicked(move |_| a.send_reaction(&mm, &e));
                rb.append(&b);
            }
            outer.append(&rb);
        }

        // Menú con clic derecho o pulsación larga.
        let a = self.clone();
        let mm = m.clone();
        let g = gtk::GestureClick::builder().button(3).build();
        let bb = bubble.clone();
        g.connect_pressed(move |_, _, _, _| a.message_menu(&bb, &mm));
        bubble.add_controller(g);
        let a = self.clone();
        let mm = m.clone();
        let lp = gtk::GestureLongPress::new();
        let bb = bubble.clone();
        lp.connect_pressed(move |_, _, _| a.message_menu(&bb, &mm));
        bubble.add_controller(lp);
        bubble.set_tooltip_text(Some("Clic derecho: responder, reaccionar, copiar…"));

        let row = gtk::ListBoxRow::builder().child(&outer).activatable(false).build();
        self.w.msg_list.append(&row);
    }

    fn message_menu(self: &Rc<Self>, parent: &gtk::Box, m: &Msg) {
        let pop = gtk::Popover::new();
        pop.set_parent(parent);
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        let bx = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).build();

        let row = gtk::Box::builder().spacing(2).build();
        for e in REACTIONS {
            let b = gtk::Button::with_label(e);
            b.add_css_class("flat");
            let a = self.clone();
            let mm = m.clone();
            let p = pop.clone();
            b.connect_clicked(move |_| {
                p.popdown();
                a.send_reaction(&mm, e);
            });
            row.append(&b);
        }
        let more_chooser = gtk::EmojiChooser::new();
        let more = gtk::MenuButton::builder().icon_name("list-add-symbolic").popover(&more_chooser).tooltip_text("Otro emoji").build();
        more.add_css_class("flat");
        {
            let a = self.clone();
            let mm = m.clone();
            let p = pop.clone();
            more_chooser.connect_emoji_picked(move |_, e| {
                p.popdown();
                a.send_reaction(&mm, e);
            });
        }
        row.append(&more);
        bx.append(&row);
        bx.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        let item = |label: &str| {
            let b = gtk::Button::builder().label(label).build();
            b.add_css_class("flat");
            b.child().and_downcast::<gtk::Label>().inspect(|l| l.set_xalign(0.0));
            bx.append(&b);
            b
        };
        let mine = m.delivery != Delivery::Incoming;
        let reply = item("Responder");
        let copy = item("Copiar texto");
        let retry = (m.delivery == Delivery::Failed).then(|| item("Reenviar"));
        let dm = (!mine && matches!(m.conv, Conv::Channel(_))).then(|| item("Mensaje directo al autor"));
        let info = (!mine).then(|| item("Ver ficha del autor"));
        let del = item("Borrar de este equipo");

        let p = pop.clone();
        let a = self.clone();
        let id = m.id;
        reply.connect_clicked(move |_| {
            p.popdown();
            a.set_reply(Some(id));
        });
        let p = pop.clone();
        let text = m.text.clone();
        let parent_w = parent.clone();
        copy.connect_clicked(move |_| {
            p.popdown();
            parent_w.clipboard().set_text(&text);
        });
        if let Some(b) = retry {
            let p = pop.clone();
            let a = self.clone();
            let mm = m.clone();
            b.connect_clicked(move |_| {
                p.popdown();
                a.st.borrow_mut().saved.messages.retain(|x| !(x.id == mm.id && x.from == mm.from));
                a.send_cmd(Command::SendText { conv: mm.conv, text: mm.text.clone(), reply_id: mm.reply_id, emoji: false });
                a.rebuild_messages();
            });
        }
        if let Some(b) = dm {
            let p = pop.clone();
            let a = self.clone();
            let from = m.from;
            b.connect_clicked(move |_| {
                p.popdown();
                a.open_dm(from);
            });
        }
        if let Some(b) = info {
            let p = pop.clone();
            let a = self.clone();
            let from = m.from;
            b.connect_clicked(move |_| {
                p.popdown();
                a.open_node_detail(from);
            });
        }
        let p = pop.clone();
        let a = self.clone();
        let mm = m.clone();
        del.connect_clicked(move |_| {
            p.popdown();
            a.st.borrow_mut().saved.messages.retain(|x| !(x.id == mm.id && x.from == mm.from && x.conv == mm.conv));
            a.save();
            a.rebuild_messages();
            a.rebuild_convs();
        });

        pop.set_child(Some(&bx));
        pop.popup();
    }

    pub(super) fn scroll_to_end(&self) {
        let sw = self.w.msg_scroll.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(60), move || {
            let a = sw.vadjustment();
            a.set_value(a.upper() - a.page_size());
        });
    }
}

/// Conversación como variante GLib, para la acción de las notificaciones.
pub(super) fn conv_variant(c: Conv) -> glib::Variant {
    match c {
        Conv::Channel(i) => format!("c{i}").to_variant(),
        Conv::Direct(n) => format!("d{n}").to_variant(),
    }
}

pub(super) fn conv_from_str(s: &str) -> Option<Conv> {
    let (k, v) = s.split_at(1);
    let n: u32 = v.parse().ok()?;
    match k {
        "c" => Some(Conv::Channel(n)),
        "d" => Some(Conv::Direct(n)),
        _ => None,
    }
}
