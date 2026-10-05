//! Persistence of SPAI settings.
use std::path::PathBuf;

pub fn config_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("HERDR_PLUGIN_CONFIG_DIR") {
        let p = PathBuf::from(dir).join("spai-settings.json");
        return Some(p);
    }
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(|h| PathBuf::from(h).join(".pi").join("agent").join("spai-settings.json"))
}

pub fn load_config_file() -> (String, String, String, u8) {
    let Some(path) = config_path() else {
        return (String::new(), String::new(), String::new(), 55);
    };
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            let key = val["api_key"].as_str().unwrap_or_default().to_string();
            let chat = val["chat_model"].as_str().unwrap_or_default().to_string();
            let embed = val["embedding_model"].as_str().unwrap_or_default().to_string();
            let thresh = val["similarity_threshold"].as_u64().unwrap_or(55) as u8;
            return (key, chat, embed, thresh);
        }
    }
    (String::new(), String::new(), String::new(), 55)
}

pub fn save_config_file(api_key: &str, chat_model: &str, embed_model: &str, thresh: u8) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::json!({
        "api_key": api_key,
        "chat_model": chat_model,
        "embedding_model": embed_model,
        "similarity_threshold": thresh,
    });
    let _ = std::fs::write(path, json.to_string());
}
