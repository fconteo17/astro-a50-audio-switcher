#![cfg(target_os = "windows")]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    a50_dock_switch_lib::run()
}
