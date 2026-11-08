# meshtastic 0.1.9 con un arreglo de Malla

Copia del crate [meshtastic](https://crates.io/crates/meshtastic) 0.1.9 (GPL-3.0).
Malla lo usa en lugar del publicado con `[patch.crates-io]` en `Cargo.toml`.

Único cambio, en `src/connections/ble_handler.rs` (`scan_peripherals`): tras buscar
nodos por Bluetooth se para la búsqueda con `adapter.stop_scan()`.

Sin él, la búsqueda se queda abierta. BlueZ rechaza la siguiente y el segundo
intento de conexión del mismo proceso falla con "Failed to find …". En Malla eso
impedía reconectar cuando el nodo se reiniciaba.

Se puede quitar esta copia cuando una versión publicada incluya el arreglo.
