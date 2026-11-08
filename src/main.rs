//! Malla: cliente de escritorio Meshtastic para Linux.

mod backend;
mod demo;
mod model;
mod store;
mod tray;
mod ui;

use adw::prelude::*;

const APP_ID: &str = "es.freemem.Malla";

fn main() -> gtk::glib::ExitCode {
    // `malla --escanear`: lista los aparatos Bluetooth y los puertos USB, sin ventana.
    if std::env::args().any(|a| a == "--escanear") {
        escanear();
        return gtk::glib::ExitCode::SUCCESS;
    }
    // `malla --probar <MAC>`: conecta por Bluetooth, resume lo recibido y sale.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--probar") {
        probar(args.get(i + 1).cloned().unwrap_or_default());
        return gtk::glib::ExitCode::SUCCESS;
    }
    let demo = std::env::args().any(|a| a == "--demo");
    if demo {
        store::set_demo();
    }
    gtk::glib::set_application_name("Malla");
    let mut flags = gtk::gio::ApplicationFlags::empty();
    if demo {
        // La demo puede convivir con una Malla de verdad abierta.
        flags |= gtk::gio::ApplicationFlags::NON_UNIQUE;
    }
    let app = adw::Application::builder().application_id(APP_ID).flags(flags).build();
    app.connect_startup(|_| gtk::Window::set_default_icon_name(APP_ID));
    app.connect_activate(|app| {
        if let Some(w) = app.active_window() {
            w.present();
            return;
        }
        let (ev_tx, ev_rx) = async_channel::unbounded();
        let cmd = if store::is_demo() { demo::spawn(ev_tx) } else { backend::spawn(ev_tx) };
        ui::build(app, cmd, ev_rx);
    });
    // Las opciones las lee Malla, no GTK.
    app.run_with_args::<&str>(&[])
}

fn escanear() {
    let ports = meshtastic::utils::stream::available_serial_ports().unwrap_or_default();
    println!("Puertos serie: {ports:?}");
    println!("Buscando por Bluetooth (6 s)...");
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    match rt.block_on(meshtastic::utils::stream::available_ble_devices(
        std::time::Duration::from_secs(6),
    )) {
        Ok(devs) => {
            for d in devs.iter().filter(|d| d.name.is_some()) {
                println!("  {}  {}", d.mac_address, d.name.as_deref().unwrap_or(""));
            }
            println!("{} aparatos en total (con y sin nombre)", devs.len());
        }
        Err(e) => println!("Error: {e}"),
    }
}

fn probar(mac: String) {
    use model::{Command, Event, Target};
    let (tx, rx) = async_channel::unbounded();
    let cmd = backend::spawn(tx);
    let _ = cmd.send(Command::Connect(Target::Ble { name: String::new(), mac }));
    let fin = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let (mut nodos, mut paquetes) = (0, 0);
    while std::time::Instant::now() < fin {
        match rx.try_recv() {
            Ok(Event::Node(n)) => {
                nodos += 1;
                println!("nodo {} {} [{}]", model::Node::id_str(n.num), n.display_name(), n.hw_model);
            }
            Ok(Event::NodePatch(..)) => paquetes += 1,
            Ok(Event::ConfigComplete) => {
                println!("== configuración completa: {nodos} nodos; escucho 20 s más");
                let f2 = std::time::Instant::now() + std::time::Duration::from_secs(20);
                while std::time::Instant::now() < f2 {
                    match rx.try_recv() {
                        Ok(Event::NodePatch(..)) => paquetes += 1,
                        Ok(e @ Event::Message(_)) => println!("{e:?}"),
                        Ok(_) => {}
                        Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
                    }
                }
                break;
            }
            Ok(e) => println!("{e:?}"),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
    println!("paquetes de la malla recibidos: {paquetes}");
    let _ = cmd.send(Command::Disconnect);
    std::thread::sleep(std::time::Duration::from_secs(2));
}
