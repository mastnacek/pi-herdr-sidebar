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
        return (String::new(), String::new(), String::new(), 85);
    };
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            let key = val["api_key"].as_str().unwrap_or_default().to_string();
            let chat = val["chat_model"].as_str().unwrap_or_default().to_string();
            let embed = val["embedding_model"].as_str().unwrap_or_default().to_string();
            let thresh = val["similarity_threshold"].as_u64().unwrap_or(85) as u8;
            return (key, chat, embed, thresh);
        }
    }
    (String::new(), String::new(), String::new(), 85)
}

pub fn save_config_file(api_key: &str, chat_model: &str, embed_model: &str, thresh: u8) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // spai-core reads the same file: keep every key this sidebar does not own
    // (`related_threshold`, ...) instead of rewriting the file from scratch.
    let mut json = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    json["api_key"] = api_key.into();
    json["chat_model"] = chat_model.into();
    json["embedding_model"] = embed_model.into();
    json["similarity_threshold"] = thresh.into();
    let _ = std::fs::write(path, json.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_keeps_related_threshold_and_unknown_keys() {
        let dir = std::env::temp_dir().join(format!("sidebar_settings_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("spai-settings.json");
        std::fs::write(&file, r#"{"related_threshold":77,"x":"keep","similarity_threshold":50}"#).unwrap();
        let old = std::env::var_os("HERDR_PLUGIN_CONFIG_DIR");
        std::env::set_var("HERDR_PLUGIN_CONFIG_DIR", &dir);
        save_config_file("k", "chat/m", "emb/m", 90);
        match old {
            Some(v) => std::env::set_var("HERDR_PLUGIN_CONFIG_DIR", v),
            None => std::env::remove_var("HERDR_PLUGIN_CONFIG_DIR"),
        }
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(v["related_threshold"], 77);
        assert_eq!(v["x"], "keep");
        assert_eq!(v["similarity_threshold"], 90);
        assert_eq!(v["embedding_model"], "emb/m");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
