#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|arg| arg == "--uninstall-clean") {
        kill_everything_lib::uninstall::perform_clean_uninstall_cli();
        return;
    }

    kill_everything_lib::run();
}
