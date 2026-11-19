// macOS: incrusta un Info.plist en el binario. Sin NSBluetoothAlwaysUsageDescription,
// macOS (TCC) mata el proceso al intentar usar Bluetooth.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let plist = format!("{dir}/data/macos/Info.plist");
        println!("cargo:rerun-if-changed=data/macos/Info.plist");
        println!("cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{plist}");
    }
}
