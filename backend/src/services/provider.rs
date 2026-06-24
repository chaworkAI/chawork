//! Unified provider config read/write, validation, masking, and connectivity probing.

use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{from_str as json_from_str, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: String,
    pub model: String,
    pub openai_base_url: String,
    pub openai_api_key: String,
    #[serde(default)]
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderConfigView {
    pub provider: String,
    pub model: String,
    pub openai_base_url: String,
    pub openai_api_key_masked: String,
    pub instructions: String,
    pub valid: bool,
    pub errors: Vec<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderModelListResult {
    pub models: Vec<String>,
    pub message: String,
    pub latency_ms: Option<u64>,
}

pub fn mask_api_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.len() <= 4 {
        return "••••".to_string();
    }
    format!("••••{}", &trimmed[trimmed.len() - 4..])
}

pub fn validate_config(config: &ProviderConfig) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    if config.provider.trim().is_empty() {
        errors.push("provider 不能为空".to_string());
    }
    if config.model.trim().is_empty() {
        errors.push("model 不能为空".to_string());
    }
    if config.openai_base_url.trim().is_empty() {
        errors.push("openai_base_url 不能为空".to_string());
    }
    if config.openai_api_key.trim().is_empty() {
        errors.push("openai_api_key 不能为空".to_string());
    }
    (errors.is_empty(), errors)
}

pub fn read_provider_json(path: &PathBuf) -> Result<ProviderConfig, String> {
    if !path.is_file() {
        return Ok(ProviderConfig {
            provider: String::new(),
            model: String::new(),
            openai_base_url: String::new(),
            openai_api_key: String::new(),
            instructions: String::new(),
        });
    }
    let raw = fs::read_to_string(path).map_err(|e| format!("读取 provider.json 失败: {e}"))?;
    let v: Value = json_from_str(&raw).map_err(|e| format!("解析 provider.json 失败: {e}"))?;
    if !v.is_object() {
        return Err("provider.json 根类型须为对象".to_string());
    }
    Ok(ProviderConfig {
        provider: v
            .get("provider")
            .and_then(|x| x.as_str())
            .unwrap_or("openai-compatible")
            .to_string(),
        model: v
            .get("model")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        openai_base_url: v
            .get("openai_base_url")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        openai_api_key: v
            .get("openai_api_key")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        instructions: v
            .get("instructions")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// Builds OpenAI-compatible probe URL: `{base_url}/models` (trailing slash tolerant).
fn models_probe_url(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("openai_base_url 为空".to_string());
    }
    Ok(format!("{trimmed}/models"))
}

pub fn write_provider_json(path: &PathBuf, config: &ProviderConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 runtime 目录失败: {e}"))?;
    }
    let existing = read_provider_json(path).unwrap_or(ProviderConfig {
        provider: String::new(),
        model: String::new(),
        openai_base_url: String::new(),
        openai_api_key: String::new(),
        instructions: String::new(),
    });
    let mut obj = serde_json::Map::new();
    let provider = config.provider.trim();
    if !provider.is_empty() {
        obj.insert("provider".to_string(), Value::String(provider.to_string()));
    }
    let model = config.model.trim();
    if !model.is_empty() {
        obj.insert("model".to_string(), Value::String(model.to_string()));
    }
    let base = config.openai_base_url.trim();
    if !base.is_empty() {
        obj.insert(
            "openai_base_url".to_string(),
            Value::String(base.to_string()),
        );
    }
    let key = config.openai_api_key.trim();
    if !key.is_empty() {
        obj.insert("openai_api_key".to_string(), Value::String(key.to_string()));
    } else if !existing.openai_api_key.trim().is_empty() {
        obj.insert(
            "openai_api_key".to_string(),
            Value::String(existing.openai_api_key),
        );
    }
    let instructions = config.instructions.trim();
    if !instructions.is_empty() {
        obj.insert(
            "instructions".to_string(),
            Value::String(instructions.to_string()),
        );
    }
    obj.insert(
        "updated_at".to_string(),
        Value::String(Utc::now().to_rfc3339()),
    );
    let out = serde_json::to_string_pretty(&Value::Object(obj)).map_err(|e| e.to_string())?;
    fs::write(path, out).map_err(|e| format!("写入 provider.json 失败: {e}"))
}

pub fn to_view(config: &ProviderConfig) -> ProviderConfigView {
    let (valid, errors) = validate_config(config);
    ProviderConfigView {
        provider: config.provider.clone(),
        model: config.model.clone(),
        openai_base_url: config.openai_base_url.clone(),
        openai_api_key_masked: mask_api_key(&config.openai_api_key),
        instructions: config.instructions.clone(),
        valid,
        errors,
        updated_at: None,
    }
}

fn parse_openai_model_ids(payload: &Value) -> Result<Vec<String>, String> {
    let data = payload
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "模型列表响应缺少 data 数组".to_string())?;

    let mut models: Vec<String> = data
        .iter()
        .filter_map(|entry| entry.get("id").and_then(|id| id.as_str()))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    models.sort();
    models.dedup();

    if models.is_empty() {
        return Err("模型列表为空".to_string());
    }

    Ok(models)
}

pub async fn list_openai_compatible_models(
    base_url: &str,
    api_key: Option<&str>,
) -> Result<ProviderModelListResult, String> {
    let probe_url = models_probe_url(base_url)?;
    let url = reqwest::Url::parse(&probe_url).map_err(|e| format!("无效的 base URL: {e}"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let start = std::time::Instant::now();
    let mut request = client.get(url);
    if let Some(key) = api_key.map(str::trim).filter(|k| !k.is_empty()) {
        request = request.bearer_auth(key);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("获取模型列表失败: {e}"))?;
    let latency_ms = start.elapsed().as_millis() as u64;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "获取模型列表失败：HTTP {}（已请求 GET /models）",
            status
        ));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|e| format!("解析模型列表失败: {e}"))?;
    let models = parse_openai_model_ids(&payload)?;

    Ok(ProviderModelListResult {
        message: format!("已获取 {} 个模型", models.len()),
        models,
        latency_ms: Some(latency_ms),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_probe_url_appends_models() {
        assert_eq!(
            models_probe_url("https://dashscope.aliyuncs.com/compatible-mode/v1").unwrap(),
            "https://dashscope.aliyuncs.com/compatible-mode/v1/models"
        );
        assert_eq!(
            models_probe_url("https://api.openai.com/v1/").unwrap(),
            "https://api.openai.com/v1/models"
        );
    }

    #[test]
    fn mask_api_key_masks_tail() {
        assert_eq!(mask_api_key("sk-abcdefghijklmnop"), "••••mnop");
        assert_eq!(mask_api_key(""), "");
    }

    #[test]
    fn validate_requires_model_url_and_api_key() {
        let cfg = ProviderConfig {
            provider: "openai-compatible".to_string(),
            model: "gpt-4".to_string(),
            openai_base_url: "https://api.example.com".to_string(),
            openai_api_key: "".to_string(),
            instructions: String::new(),
        };
        let (ok, errs) = validate_config(&cfg);
        assert!(!ok);
        assert!(errs.iter().any(|e| e.contains("openai_api_key")));
    }

    #[test]
    fn parses_openai_model_ids_from_data_array() {
        let payload = serde_json::json!({
            "object": "list",
            "data": [
                { "id": "gpt-4.1", "object": "model" },
                { "id": "gpt-4.1-mini", "object": "model" },
                { "id": "gpt-4.1", "object": "model" },
                { "object": "model" }
            ]
        });
        let models = parse_openai_model_ids(&payload).unwrap();
        assert_eq!(models, vec!["gpt-4.1", "gpt-4.1-mini"]);
    }
}
