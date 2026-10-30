#[tauri::command]
fn read_dir(path: String) -> Vec<String> {
std::fs::read_dir(path).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect()
}
