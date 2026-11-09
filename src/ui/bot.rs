//! Respuestas automáticas: contestar solo a mensajes que contienen una palabra.
//! Es la versión sencilla de los bots de otros clientes.

use super::*;
use crate::store::AutoRule;

/// No contestar dos veces al mismo nodo en este tiempo (segundos).
const PAUSE_PER_NODE: u32 = 60;
/// No contestar a mensajes más viejos que esto (los que llegan al conectar).
const MAX_AGE: u32 = 120;

const SCOPES: [&str; 3] = ["Solo mensajes directos", "Solo canales", "Directos y canales"];

/// Rellena los huecos de la respuesta con datos del mensaje recibido.
pub(super) fn fill(reply: &str, name: &str, m: &Msg) -> String {
    reply
        .replace("{nombre}", name)
        .replace("{snr}", &m.snr.map(|v| format!("{v:.1}")).unwrap_or_else(|| "?".into()))
        .replace("{rssi}", &m.rssi.map(|v| v.to_string()).unwrap_or_else(|| "?".into()))
        .replace("{saltos}", &m.hops.map(hops_text).unwrap_or_else(|| "saltos desconocidos".into()))
        .replace("{hora}", &fmt_time(now()))
}

/// La primera regla que encaja con el mensaje.
pub(super) fn matching<'a>(rules: &'a [AutoRule], m: &Msg) -> Option<&'a AutoRule> {
    let direct = matches!(m.conv, Conv::Direct(_));
    let text = m.text.to_lowercase();
    rules.iter().find(|r| {
        let k = r.keyword.trim().to_lowercase();
        let scope_ok = match r.scope {
            0 => direct,
            1 => !direct,
            _ => true,
        };
        !k.is_empty() && !r.reply.trim().is_empty() && scope_ok && text.contains(&k)
    })
}

impl App {
    pub(super) fn maybe_auto_reply(self: &Rc<Self>, m: &Msg) {
        let reply = {
            let mut st = self.st.borrow_mut();
            let prefs = &st.saved.prefs;
            if !prefs.auto_on || !st.connected || m.from == st.my || now().saturating_sub(m.time) > MAX_AGE {
                return;
            }
            let Some(rule) = matching(&prefs.auto_rules, m) else { return };
            let reply = rule.reply.clone();
            if st.auto_last.get(&m.from).is_some_and(|t| now().saturating_sub(*t) < PAUSE_PER_NODE) {
                return;
            }
            st.auto_last.insert(m.from, now());
            let name = st.nodes.get(&m.from).map(|n| n.short()).unwrap_or_else(|| st.name_of(m.from));
            fill(&reply, &name, m)
        };
        let text: String = reply.chars().take(200).collect();
        self.send_cmd(Command::SendText { conv: m.conv, text, reply_id: Some(m.id), emoji: false });
    }

    pub(super) fn auto_page(self: &Rc<Self>) -> adw::NavigationPage {
        let page = adw::PreferencesPage::new();
        let prefs = self.st.borrow().saved.prefs.clone();

        let g = adw::PreferencesGroup::builder()
            .description(
                "Malla contesta sola cuando un mensaje contiene una palabra. Solo mientras está abierta y conectada, \
                 y como mucho una vez por minuto a cada nodo. Cada respuesta ocupa el aire de todos: úsalas con cabeza.",
            )
            .build();
        let on = adw::SwitchRow::builder().title("Activar las respuestas automáticas").active(prefs.auto_on).build();
        g.add(&on);
        page.add(&g);
        let a = self.clone();
        on.connect_active_notify(move |r| {
            a.st.borrow_mut().saved.prefs.auto_on = r.is_active();
            a.save();
            a.settings_dirty.set(true);
        });

        let rules = adw::PreferencesGroup::builder()
            .title("Reglas")
            .description("En la respuesta puedes usar {nombre}, {snr}, {rssi}, {saltos} y {hora}.")
            .build();
        let add = gtk::Button::builder().icon_name("list-add-symbolic").tooltip_text("Añadir regla").valign(gtk::Align::Center).build();
        add.add_css_class("flat");
        rules.set_header_suffix(Some(&add));
        page.add(&rules);

        // Cada regla es un grupo de filas; la lista se reconstruye al añadir o borrar.
        let holder: Rc<RefCell<Vec<adw::PreferencesGroup>>> = Rc::new(RefCell::new(vec![]));
        let rebuild: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        {
            let a = self.clone();
            let page = page.clone();
            let holder = holder.clone();
            let rebuild2 = rebuild.clone();
            *rebuild.borrow_mut() = Some(Box::new(move || {
                for g in holder.borrow_mut().drain(..) {
                    page.remove(&g);
                }
                let list = a.st.borrow().saved.prefs.auto_rules.clone();
                for (i, r) in list.iter().enumerate() {
                    let g = adw::PreferencesGroup::new();
                    let key = adw::EntryRow::builder().title("Si el mensaje contiene").text(&r.keyword).build();
                    let reply = adw::EntryRow::builder().title("Contestar").text(&r.reply).build();
                    let scope = adw::ComboRow::builder().title("Dónde").model(&gtk::StringList::new(&SCOPES)).selected(r.scope.min(2) as u32).build();
                    let del = gtk::Button::builder().icon_name("user-trash-symbolic").tooltip_text("Borrar la regla").valign(gtk::Align::Center).build();
                    del.add_css_class("flat");
                    scope.add_suffix(&del);
                    g.add(&key);
                    g.add(&reply);
                    g.add(&scope);
                    let aa = a.clone();
                    key.connect_changed(move |e| {
                        if let Some(r) = aa.st.borrow_mut().saved.prefs.auto_rules.get_mut(i) {
                            r.keyword = e.text().to_string();
                        }
                        aa.save();
                    });
                    let aa = a.clone();
                    reply.connect_changed(move |e| {
                        if let Some(r) = aa.st.borrow_mut().saved.prefs.auto_rules.get_mut(i) {
                            r.reply = e.text().to_string();
                        }
                        aa.save();
                    });
                    let aa = a.clone();
                    scope.connect_selected_notify(move |c| {
                        if let Some(r) = aa.st.borrow_mut().saved.prefs.auto_rules.get_mut(i) {
                            r.scope = c.selected() as u8;
                        }
                        aa.save();
                    });
                    let aa = a.clone();
                    let rb = rebuild2.clone();
                    del.connect_clicked(move |_| {
                        {
                            let mut st = aa.st.borrow_mut();
                            if i < st.saved.prefs.auto_rules.len() {
                                st.saved.prefs.auto_rules.remove(i);
                            }
                        }
                        aa.save();
                        let rb = rb.clone();
                        glib::idle_add_local_once(move || {
                            if let Some(f) = rb.borrow().as_ref() {
                                f();
                            }
                        });
                    });
                    page.add(&g);
                    holder.borrow_mut().push(g);
                }
            }));
        }
        if let Some(f) = rebuild.borrow().as_ref() {
            f();
        }
        let a = self.clone();
        let rb = rebuild.clone();
        add.connect_clicked(move |_| {
            a.st.borrow_mut().saved.prefs.auto_rules.push(AutoRule { keyword: String::new(), reply: String::new(), scope: 0 });
            a.save();
            if let Some(f) = rb.borrow().as_ref() {
                f();
            }
        });

        let header = adw::HeaderBar::new();
        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&header);
        tv.set_content(Some(&page));
        adw::NavigationPage::builder().title("Respuestas automáticas").child(&tv).build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(text: &str, conv: Conv) -> Msg {
        Msg {
            id: 1,
            conv,
            from: 7,
            text: text.into(),
            time: 0,
            delivery: Delivery::Incoming,
            error: None,
            snr: Some(6.25),
            rssi: Some(-90),
            hops: Some(2),
            reply_id: None,
            emoji: false,
        }
    }

    #[test]
    fn reglas() {
        let rules = vec![
            AutoRule { keyword: "ping".into(), reply: "pong {nombre} {snr}".into(), scope: 0 },
            AutoRule { keyword: "hola".into(), reply: "¡hola!".into(), scope: 1 },
        ];
        let dm = msg("PING?", Conv::Direct(7));
        let r = matching(&rules, &dm).expect("ping en directo");
        assert_eq!(fill(&r.reply, "ABCD", &dm), "pong ABCD 6.2");
        assert!(matching(&rules, &msg("ping", Conv::Channel(0))).is_none(), "ping solo en directos");
        assert!(matching(&rules, &msg("Hola a todos", Conv::Channel(0))).is_some());
        assert!(matching(&rules, &msg("hola", Conv::Direct(7))).is_none());
    }
}
