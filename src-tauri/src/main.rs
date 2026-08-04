// This is a graphical client in both debug and release builds. Keeping the
// Windows GUI subsystem prevents a separate console window from appearing
// when testers launch the executable directly.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    pixnya_lib::run()
}
