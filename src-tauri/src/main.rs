#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(all(not(debug_assertions), not(feature = "custom-protocol")))]
compile_error!("Release builds require --features custom-protocol to embed the offline frontend.");

fn main() {
    cleancore_lib::run();
}
