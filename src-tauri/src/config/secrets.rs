const SERVICE: &str = "com.easygit.app";

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account).map_err(|e| format!("无法访问系统钥匙串: {e}"))
}

pub fn read_secret(account: &str) -> Result<Option<String>, String> {
    match entry(account)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("读取凭据失败: {e}")),
    }
}

#[tauri::command]
pub fn set_secret(account: String, value: String) -> Result<(), String> {
    entry(&account)?
        .set_password(&value)
        .map_err(|e| format!("保存凭据失败: {e}"))
}

#[tauri::command]
pub fn has_secret(account: String) -> Result<bool, String> {
    Ok(read_secret(&account)?.is_some())
}

#[tauri::command]
pub fn delete_secret(account: String) -> Result<(), String> {
    match entry(&account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("删除凭据失败: {e}")),
    }
}
