#[allow(dead_code)]
mod catalog_src;
#[allow(dead_code)]
mod state;

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
