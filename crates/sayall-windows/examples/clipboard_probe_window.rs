//! Native receiving-surface smoke test only. This does not emulate Doubao or BLE.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#[cfg(windows)]
fn main() {
    std::env::set_var(
        "SAYALL_GATT_LOG",
        std::env::temp_dir().join("sayall-clipboard-probe-smoke.log"),
    );
    sayall_windows::clipboard_probe::set_enabled(true, false).expect("native receiving window");
    sayall_windows::clipboard_probe::begin();
    std::thread::sleep(std::time::Duration::from_secs(20));
    sayall_windows::clipboard_probe::release();
    std::thread::sleep(std::time::Duration::from_secs(40));
    sayall_windows::clipboard_probe::shutdown();
}
#[cfg(not(windows))]
fn main() {}
