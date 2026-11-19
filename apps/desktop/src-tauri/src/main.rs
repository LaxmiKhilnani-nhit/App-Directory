// Never attach a console window, including in debug builds.
#![windows_subsystem = "windows"]

fn main() {
    app_directory_lib::run()
}
