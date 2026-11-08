//! Icono en la bandeja del sistema (StatusNotifierItem), como Telegram o Spotify.
//! Corre en su propio hilo con tokio; habla con la UI por canales.

use ksni::menu::{MenuItem, StandardItem};
use ksni::TrayMethods;
use tokio::sync::mpsc;

/// Lo que el usuario pide desde el icono.
#[derive(Debug)]
pub enum Action {
    /// El icono está en la barra (true) o no hay bandeja disponible (false).
    Ready(bool),
    Toggle,
    Show,
    Connect,
    Disconnect,
    Quit,
}

/// Estado que la UI manda al icono.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Status {
    pub unread: u32,
    pub connected: bool,
}

struct MallaTray {
    st: Status,
    icon: ksni::Icon,
    icon_alert: ksni::Icon,
    ui: async_channel::Sender<Action>,
}

impl MallaTray {
    fn send(&self, a: Action) {
        let _ = self.ui.try_send(a);
    }
}

impl ksni::Tray for MallaTray {
    fn id(&self) -> String {
        "es.freemem.Malla".into()
    }

    fn title(&self) -> String {
        "Malla".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Communications
    }

    fn status(&self) -> ksni::Status {
        if self.st.unread > 0 {
            ksni::Status::NeedsAttention
        } else {
            ksni::Status::Active
        }
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![if self.st.unread > 0 { self.icon_alert.clone() } else { self.icon.clone() }]
    }

    fn attention_icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon_alert.clone()]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Malla".into(),
            description: self.status_text(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(Action::Toggle);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Mostrar Malla".into(),
                activate: Box::new(|t: &mut Self| t.send(Action::Show)),
                ..Default::default()
            }
            .into(),
            StandardItem { label: self.status_text(), enabled: false, ..Default::default() }.into(),
            MenuItem::Separator,
            if self.st.connected {
                StandardItem {
                    label: "Desconectar".into(),
                    activate: Box::new(|t: &mut Self| t.send(Action::Disconnect)),
                    ..Default::default()
                }
            } else {
                StandardItem {
                    label: "Conectar…".into(),
                    activate: Box::new(|t: &mut Self| t.send(Action::Connect)),
                    ..Default::default()
                }
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Salir".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|t: &mut Self| t.send(Action::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

impl MallaTray {
    fn status_text(&self) -> String {
        let c = if self.st.connected { "Conectado" } else { "Sin conexión" };
        match self.st.unread {
            0 => c.to_string(),
            1 => format!("{c} · 1 mensaje sin leer"),
            n => format!("{c} · {n} mensajes sin leer"),
        }
    }
}

/// Arranca el icono. `rgba` es el icono en RGBA sin premultiplicar, de `size`×`size`.
pub fn spawn(rgba: Vec<u8>, size: i32, ui: async_channel::Sender<Action>) -> mpsc::UnboundedSender<Status> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Status>();
    std::thread::Builder::new()
        .name("bandeja".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(_) => {
                    let _ = ui.try_send(Action::Ready(false));
                    return;
                }
            };
            rt.block_on(async move {
                let tray = MallaTray {
                    st: Status { unread: 0, connected: false },
                    icon: to_argb(&rgba, size, false),
                    icon_alert: to_argb(&rgba, size, true),
                    ui: ui.clone(),
                };
                let handle = match tray.spawn().await {
                    Ok(h) => {
                        let _ = ui.try_send(Action::Ready(true));
                        h
                    }
                    Err(_) => {
                        let _ = ui.try_send(Action::Ready(false));
                        return;
                    }
                };
                while let Some(st) = rx.recv().await {
                    handle.update(|t| t.st = st).await;
                }
            });
        })
        .expect("no se pudo crear el hilo de la bandeja");
    tx
}

/// RGBA → ARGB (orden de red), con un punto rojo arriba a la derecha si `alert`.
fn to_argb(rgba: &[u8], size: i32, alert: bool) -> ksni::Icon {
    let s = size as f32;
    let (cx, cy, r) = (s * 0.76, s * 0.24, s * 0.22);
    let mut data = Vec::with_capacity(rgba.len());
    for (i, px) in rgba.chunks_exact(4).enumerate() {
        let (x, y) = ((i as i32 % size) as f32 + 0.5, (i as i32 / size) as f32 + 0.5);
        let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
        let p = if alert && d <= r {
            [255, 224, 27, 36] // rojo
        } else if alert && d <= r + s * 0.05 {
            [255, 255, 255, 255] // borde blanco
        } else {
            [px[3], px[0], px[1], px[2]]
        };
        data.extend_from_slice(&p);
    }
    ksni::Icon { width: size, height: size, data }
}
