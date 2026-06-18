use crate::config::{self, load_settings, UserRule};
use crate::dns_check::domain_resolvable;
use crate::mihomo::reload_running_config;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionItem {
  pub domain: String,
  pub host: String,
  pub network: String,
  pub rule: String,
  pub chains: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AddRuleResult {
  pub rules: Vec<UserRule>,
  pub warning: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConnectionsResponse {
  #[serde(default)]
  connections: Option<Vec<RawConnection>>,
}

#[derive(Debug, Deserialize)]
struct RawConnection {
  metadata: ConnectionMetadata,
  rule: String,
  chains: Vec<String>,
  #[serde(default)]
  rule_payload: String,
}

#[derive(Debug, Deserialize)]
struct ConnectionMetadata {
  host: String,
  #[allow(dead_code)]
  destination_ip: String,
  network: String,
}

pub fn fetch_connections() -> Result<Vec<ConnectionItem>, String> {
  let url = format!("http://127.0.0.1:{}/connections", config::API_PORT);
  let client = reqwest::blocking::Client::new();
  let response = client
    .get(&url)
    .header("Authorization", format!("Bearer {}", config::API_SECRET))
    .send()
    .map_err(|e| format!("读取连接失败，请确认分流已开启: {e}"))?;
  if !response.status().is_success() {
    return Err(format!("Clash API 返回 {}", response.status()));
  }
  let body: ConnectionsResponse = response.json().map_err(|e| e.to_string())?;
  let connections = body.connections.unwrap_or_default();
  let mut seen = HashSet::new();
  let mut items = Vec::new();
  for conn in connections {
    let host = conn.metadata.host.clone();
    if host.is_empty() || !seen.insert(host.clone()) {
      continue;
    }
    let domain = extract_domain(&host);
    items.push(ConnectionItem {
      domain,
      host,
      network: conn.metadata.network,
      rule: if conn.rule_payload.is_empty() {
        conn.rule
      } else {
        format!("{} / {}", conn.rule, conn.rule_payload)
      },
      chains: conn.chains,
    });
  }
  items.truncate(30);
  Ok(items)
}

fn extract_domain(host: &str) -> String {
  host.split(':').next().unwrap_or(host).to_string()
}

pub fn normalize_domain(input: &str) -> Option<String> {
  let trimmed = input.trim();
  if trimmed.is_empty() {
    return None;
  }
  let without_scheme = trimmed
    .trim_start_matches("https://")
    .trim_start_matches("http://");
  let host = without_scheme.split('/').next()?.split(':').next()?.trim();
  if host.is_empty() || host.contains(' ') {
    return None;
  }
  Some(host.to_lowercase())
}

fn apply_rules(rules: &[UserRule]) -> Result<(), String> {
  let settings = load_settings();
  reload_running_config(&settings).map_err(|error| {
    format!("{error}。请先关闭再重新开启分流。")
  })?;
  let _ = rules;
  Ok(())
}

pub fn upsert_user_rule(domain: &str, action: &str) -> Result<AddRuleResult, String> {
  let domain = normalize_domain(domain).ok_or_else(|| "域名无效".to_string())?;
  if action != "proxy" && action != "direct" {
    return Err("action 只能是 proxy 或 direct".into());
  }
  let mut rules = config::load_user_rules();
  rules.retain(|rule| rule.domain != domain);
  rules.insert(
    0,
    UserRule {
      domain: domain.clone(),
      action: action.to_string(),
    },
  );
  config::save_user_rules(&rules)?;
  apply_rules(&rules)?;

  let warning = if action == "direct" && !domain_resolvable(&domain) {
    Some(format!(
      "规则已生效，但 {domain} 当前无法解析。这通常是公司内网域名，需要先连接公司 VPN 或检查 hosts。"
    ))
  } else {
    None
  };

  Ok(AddRuleResult { rules, warning })
}

pub fn remove_user_rule(domain: &str) -> Result<Vec<UserRule>, String> {
  let domain = normalize_domain(domain).ok_or_else(|| "域名无效".to_string())?;
  let mut rules = config::load_user_rules();
  rules.retain(|rule| rule.domain != domain);
  config::save_user_rules(&rules)?;
  apply_rules(&rules)?;
  Ok(rules)
}
