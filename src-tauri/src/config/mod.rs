pub mod schema;
pub mod secrets;

use std::fs;
use std::path::PathBuf;

pub use schema::AppConfig;

fn config_path() -> Result<PathBuf, String> {
    let dir = dirs::config_dir()
        .ok_or_else(|| "无法定位系统配置目录".to_string())?
        .join("easy-git");
    fs::create_dir_all(&dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    Ok(dir.join("config.json"))
}

#[tauri::command]
pub fn get_settings() -> Result<AppConfig, String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取配置失败: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析配置失败: {e}"))
}

#[tauri::command]
pub fn save_settings(config: AppConfig) -> Result<(), String> {
    let path = config_path()?;
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(&config).map_err(|e| format!("序列化配置失败: {e}"))?;
    fs::write(&tmp, json).map_err(|e| format!("写入配置失败: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("保存配置失败: {e}"))
}
