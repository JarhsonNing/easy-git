use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub projects: Vec<ProjectEntry>,
    pub gitlab: GitLabConfig,
    pub ai: AiConfig,
    pub git: GitConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
            gitlab: GitLabConfig::default(),
            ai: AiConfig::default(),
            git: GitConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntry {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GitLabConfig {
    pub instances: Vec<GitLabInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLabInstance {
    pub id: String,
    pub name: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiConfig {
    pub active_profile_id: Option<String>,
    pub profiles: Vec<AiProfile>,
}

impl Default for AiConfig {
    fn default() -> Self {
        let ollama = AiProfile {
            id: "ollama".to_string(),
            name: "Ollama (本地)".to_string(),
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            model: "qwen2.5-coder:7b".to_string(),
            temperature: 0.2,
            max_tokens: 2048,
        };
        Self {
            active_profile_id: Some(ollama.id.clone()),
            profiles: vec![ollama],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
}

impl Default for AiProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "本地模型".to_string(),
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            model: "qwen2.5-coder:7b".to_string(),
            temperature: 0.2,
            max_tokens: 2048,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GitConfig {
    pub page_size: u32,
    pub max_lanes: u32,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            page_size: 500,
            max_lanes: 32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn parses_config_without_projects() {
        let raw = r#"{
            "gitlab": { "instances": [] },
            "ai": { "activeProfileId": null, "profiles": [] },
            "git": { "pageSize": 50, "maxLanes": 32 }
        }"#;
        let config: AppConfig = serde_json::from_str(raw).unwrap();
        assert!(config.projects.is_empty());
        assert_eq!(config.git.page_size, 50);
    }
}
