//! Lista de nodos: orden, filtros, distancia y acciones sobre cada nodo.

use super::*;

pub(super) struct NodesUi {
    pub root: gtk::ScrolledWindow,
    pub list: gtk::ListBox,
    pub count: gtk::Label,
    pub extra: NodesExtra,
}

pub(super) struct NodesExtra {
    search: gtk::SearchEntry,
    sort: gtk::DropDown,
    active: gtk::CheckButton,
    mqtt: gtk::CheckButton,
    fav: gtk::CheckButton,
    ignored: gtk::CheckButton,
    direct: gtk::CheckButton,
    pos: gtk::CheckButton,
}

const SORTS: [(&str, NodeSort); 5] = [
    ("Último contacto", NodeSort::LastHeard),
    ("Distancia", NodeSort::Distance),
    ("Nombre", NodeSort::Name),
    ("Señal (SNR)", NodeSort::Snr),
    ("Saltos", NodeSort::Hops),
];

pub(super) fn build_page() -> NodesUi {
    let search = gtk::SearchEntry::builder().placeholder_text("Buscar por nombre, id o número").hexpand(true).build();
    let names: Vec<&str> = SORTS.iter().map(|s| s.0).collect();
    let sort = gtk::DropDown::from_strings(&names);
    sort.set_tooltip_text(Some("Ordenar por"));

    let active = gtk::CheckButton::with_label("Solo activos (últimas 2 h)");
    let mqtt = gtk::CheckButton::with_label("Ocultar los que llegan por MQTT");
    let fav = gtk::CheckButton::with_label("Solo favoritos");
    let ignored = gtk::CheckButton::with_label("Mostrar ignorados");
    let direct = gtk::CheckButton::with_label("Solo los que oigo directamente (0 saltos)");
    let pos = gtk::CheckButton::with_label("Solo los que tienen posición");
    let fbox = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).build();
    for c in [&active, &direct, &pos, &mqtt, &fav, &ignored] {
        fbox.append(c);
    }
    let fpop = gtk::Popover::builder().child(&fbox).build();
    let filter_btn = gtk::MenuButton::builder().label("Filtros").popover(&fpop).build();

    let count = gtk::Label::new(None);
    count.add_css_class("dim-label");
    let top = gtk::Box::builder().spacing(8).build();
    top.append(&search);
    top.append(&sort);
    top.append(&filter_btn);
    top.append(&count);

    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
    list.add_css_class("boxed-list");
    let bx = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .build();
    bx.append(&top);
    bx.append(&list);
    let root = gtk::ScrolledWindow::builder()
        .child(&adw::Clamp::builder().maximum_size(980).child(&bx).build())
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    NodesUi { root, list, count, extra: NodesExtra { search, sort, active, mqtt, fav, ignored, direct, pos } }
}

pub(super) fn connect_signals(app: &Rc<App>, x: &NodesExtra) {
    let a = app.clone();
    x.search.connect_search_changed(move |s| {
        a.st.borrow_mut().nf.text = s.text().to_lowercase();
        a.rebuild_nodes();
    });
    let a = app.clone();
    x.sort.connect_selected_notify(move |d| {
        a.st.borrow_mut().nf.sort = SORTS.get(d.selected() as usize).map(|s| s.1).unwrap_or_default();
        a.rebuild_nodes();
    });
    let pairs: [(&gtk::CheckButton, fn(&mut NodeFilter, bool)); 6] = [
        (&x.active, |f, v| f.only_active = v),
        (&x.direct, |f, v| f.only_direct = v),
        (&x.pos, |f, v| f.only_pos = v),
        (&x.mqtt, |f, v| f.hide_mqtt = v),
        (&x.fav, |f, v| f.only_fav = v),
        (&x.ignored, |f, v| f.show_ignored = v),
    ];
    for (c, set) in pairs {
        let a = app.clone();
        c.connect_toggled(move |c| {
            set(&mut a.st.borrow_mut().nf, c.is_active());
            a.rebuild_nodes();
        });
    }
}

impl App {
    pub(super) fn rebuild_nodes(self: &Rc<Self>) {
        let list = &self.w.nodes_list;
        list.remove_all();
        let st = self.st.borrow();
        let my = st.my;
        let me = st.my_pos();
        let t = now();
        let f = &st.nf;
        let dist = |n: &Node| match (me, n.lat, n.lon) {
            (Some(m), Some(la), Some(lo)) if n.num != my => Some(distance_bearing(m, (la, lo))),
            _ => None,
        };
        let mut nodes: Vec<&Node> = st
            .nodes
            .values()
            .filter(|n| {
                (f.text.is_empty()
                    || n.long_name.to_lowercase().contains(&f.text)
                    || n.short_name.to_lowercase().contains(&f.text)
                    || Node::id_str(n.num).contains(&f.text)
                    || n.num.to_string() == f.text)
                    && (!f.only_direct || n.num == my || (n.hops_away == Some(0) && !n.via_mqtt))
                    && (!f.only_pos || n.lat.is_some())
                    && (!f.only_active || n.num == my || t.saturating_sub(n.last_heard) < 7200)
                    && (!f.hide_mqtt || !n.via_mqtt)
                    && (!f.only_fav || n.favorite || n.num == my)
                    && (f.show_ignored || !n.ignored)
            })
            .collect();
        nodes.sort_by(|a, b| {
            let first = (b.num == my).cmp(&(a.num == my)).then(b.favorite.cmp(&a.favorite));
            first.then_with(|| match f.sort {
                NodeSort::LastHeard => b.last_heard.cmp(&a.last_heard),
                NodeSort::Name => a.display_name().to_lowercase().cmp(&b.display_name().to_lowercase()),
                NodeSort::Snr => b.snr.unwrap_or(-99.0).total_cmp(&a.snr.unwrap_or(-99.0)),
                NodeSort::Hops => a.hops_away.unwrap_or(99).cmp(&b.hops_away.unwrap_or(99)),
                NodeSort::Distance => {
                    let da = dist(a).map(|d| d.0).unwrap_or(f64::MAX);
                    let db = dist(b).map(|d| d.0).unwrap_or(f64::MAX);
                    da.total_cmp(&db)
                }
            })
        });
        let online = st.nodes.values().filter(|n| t.saturating_sub(n.last_heard) < 7200).count();
        self.w.nodes_count.set_text(&format!("{} nodos · {online} activos", st.nodes.len()));

        if nodes.is_empty() {
            let r = adw::ActionRow::builder()
                .title(if st.nodes.is_empty() && !st.connected { "Conéctate a tu nodo para ver la malla" } else { "Ningún nodo con estos filtros" })
                .build();
            list.append(&r);
        }

        for n in nodes {
            let mut sub = vec![Node::id_str(n.num)];
            if n.num == my {
                sub.push("este nodo".into());
            } else {
                sub.push(ago(n.last_heard));
                sub.push(if n.via_mqtt {
                    "por MQTT".into()
                } else {
                    n.hops_away.map(hops_text).unwrap_or_else(|| "saltos ?".into())
                });
                if let Some(s) = n.snr {
                    sub.push(format!("SNR {s:.1}"));
                }
                if let Some((d, b)) = dist(n) {
                    sub.push(format!("{} {}", fmt_distance(d), compass(b)));
                }
            }
            if let Some(b) = n.battery {
                sub.push(battery_text(b));
            }
            if let Some(tc) = n.temperature {
                sub.push(format!("{tc:.1} °C"));
            }

            let mut title = n.display_name();
            if n.favorite {
                title = format!("★ {title}");
            }
            if n.ignored {
                title = format!("{title} (ignorado)");
            }
            let row = adw::ActionRow::builder()
                .title(title)
                .subtitle(sub.join(" · "))
                .use_markup(false)
                .activatable(true)
                .build();
            row.add_prefix(&avatar(&n.short(), n.num, false));

            let num = n.num;
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
            let menu = gtk::MenuButton::builder().icon_name("view-more-symbolic").valign(gtk::Align::Center).build();
            menu.add_css_class("flat");
            let a = self.clone();
            menu.set_create_popup_func(move |mb| {
                mb.set_popover(Some(&a.node_menu(num)));
            });
            row.add_suffix(&menu);
            let a = self.clone();
            row.connect_activated(move |_| a.open_node_detail(num));
            list.append(&row);
        }
    }

    /// Menú de acciones sobre un nodo (lista, mapa y ficha).
    pub(super) fn node_menu(self: &Rc<Self>, num: u32) -> gtk::Popover {
        let pop = gtk::Popover::new();
        let bx = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).build();
        let (my, fav, ign, has_pos, name) = {
            let st = self.st.borrow();
            let n = st.nodes.get(&num);
            (
                st.my,
                n.map(|n| n.favorite).unwrap_or(false),
                n.map(|n| n.ignored).unwrap_or(false),
                n.map(|n| n.lat.is_some()).unwrap_or(false),
                st.name_of(num),
            )
        };
        let add = |label: &str, f: Box<dyn Fn(&Rc<App>)>| {
            let b = gtk::Button::builder().label(label).build();
            b.add_css_class("flat");
            b.child().and_downcast::<gtk::Label>().inspect(|l| l.set_xalign(0.0));
            let a = self.clone();
            let p = pop.clone();
            b.connect_clicked(move |_| {
                p.popdown();
                f(&a);
            });
            bx.append(&b);
        };
        add("Ver ficha", Box::new(move |a| a.open_node_detail(num)));
        if has_pos {
            add("Ver en el mapa", Box::new(move |a| a.show_node_on_map(num)));
        }
        if num != my {
            add("Mensaje directo", Box::new(move |a| a.open_dm(num)));
            add("Traceroute", Box::new(move |a| a.send_cmd(Command::Traceroute(num))));
            add("Pedir posición", Box::new(move |a| a.send_cmd(Command::RequestPosition(num))));
            add("Intercambiar datos de usuario", Box::new(move |a| a.send_cmd(Command::RequestUser(num))));
            add(
                if fav { "Quitar de favoritos" } else { "Añadir a favoritos" },
                Box::new(move |a| a.set_favorite(num, !fav)),
            );
            add(
                if ign { "Dejar de ignorar" } else { "Ignorar" },
                Box::new(move |a| a.set_ignored(num, !ign)),
            );
            let name2 = name.clone();
            add(
                "Quitar de la lista del nodo…",
                Box::new(move |a| {
                    let a2 = a.clone();
                    a.confirm(
                        "¿Quitar el nodo?",
                        &format!("Se borra {name2} de la lista de tu radio. Volverá a aparecer si se le oye de nuevo."),
                        "Quitar",
                        move || {
                            a2.admin(vec![protobufs::admin_message::PayloadVariant::RemoveByNodenum(num)]);
                            a2.st.borrow_mut().nodes.remove(&num);
                            a2.nodes_dirty.set(true);
                        },
                    );
                }),
            );
        }
        pop.set_child(Some(&bx));
        pop
    }

    pub(super) fn set_favorite(&self, num: u32, on: bool) {
        use protobufs::admin_message::PayloadVariant as P;
        self.admin(vec![if on { P::SetFavoriteNode(num) } else { P::RemoveFavoriteNode(num) }]);
        if let Some(n) = self.st.borrow_mut().nodes.get_mut(&num) {
            n.favorite = on;
        }
        self.nodes_dirty.set(true);
    }

    pub(super) fn set_ignored(&self, num: u32, on: bool) {
        use protobufs::admin_message::PayloadVariant as P;
        self.admin(vec![if on { P::SetIgnoredNode(num) } else { P::RemoveIgnoredNode(num) }]);
        if let Some(n) = self.st.borrow_mut().nodes.get_mut(&num) {
            n.ignored = on;
        }
        self.nodes_dirty.set(true);
    }
}
