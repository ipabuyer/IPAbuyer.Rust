//! 系统主题查询命令。

#[tauri::command]
pub fn system_theme() -> &'static str {
    crate::system_theme::current_system_theme()
}
