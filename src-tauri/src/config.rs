use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use crate::dns_check::resolve_system_ipv4;

pub const MIXED_PORT: u16 = 17890;
pub const API_PORT: u16 = 19090;
pub const API_SECRET: &str = "flowroute-local";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
  Http,
  Socks5,
}

impl Default for ProxyType {
  fn default() -> Self {
    Self::Socks5
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamProxy {
  #[serde(rename = "type")]
  pub proxy_type: ProxyType,
  pub host: String,
  pub port: u16,
}

impl Default for UpstreamProxy {
  fn default() -> Self {
    Self {
      proxy_type: ProxyType::Socks5,
      host: "127.0.0.1".into(),
      port: 7890,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
  pub upstream: UpstreamProxy,
}

impl Default for AppSettings {
  fn default() -> Self {
    Self {
      upstream: UpstreamProxy::default(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRule {
  pub domain: String,
  pub action: String,
}

pub fn app_dirs() -> ProjectDirs {
  ProjectDirs::from("com", "black", "flowroute")
    .expect("failed to resolve app directories")
}

pub fn data_dir() -> PathBuf {
  let dir = app_dirs().data_dir().to_path_buf();
  fs::create_dir_all(&dir).ok();
  dir
}

pub fn settings_path() -> PathBuf {
  data_dir().join("settings.json")
}

pub fn rules_dir() -> PathBuf {
  let dir = data_dir().join("rules");
  fs::create_dir_all(&dir).ok();
  dir
}

pub fn config_path() -> PathBuf {
  data_dir().join("config.yaml")
}

pub fn load_settings() -> AppSettings {
  let path = settings_path();
  if path.exists() {
    if let Ok(text) = fs::read_to_string(&path) {
      if let Ok(settings) = serde_json::from_str(&text) {
        return settings;
      }
    }
  }
  AppSettings::default()
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
  let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
  fs::write(settings_path(), text).map_err(|e| e.to_string())
}

pub fn load_user_rules() -> Vec<UserRule> {
  let path = rules_dir().join("user-rules.json");
  if !path.exists() {
    return Vec::new();
  }
  fs::read_to_string(path)
    .ok()
    .and_then(|text| serde_json::from_str(&text).ok())
    .unwrap_or_default()
}

pub fn save_user_rules(rules: &[UserRule]) -> Result<(), String> {
  let path = rules_dir().join("user-rules.json");
  let text = serde_json::to_string_pretty(rules).map_err(|e| e.to_string())?;
  fs::write(path, text).map_err(|e| e.to_string())
}

fn parent_suffix(domain: &str) -> Option<String> {
  let domain = domain.trim_start_matches("*.");
  let parts: Vec<&str> = domain.split('.').collect();
  if parts.len() > 2 {
    Some(parts[parts.len() - 2..].join("."))
  } else {
    None
  }
}

pub fn build_inline_rules(rules: &[UserRule]) -> String {
  let mut lines = Vec::new();
  for rule in rules {
    let domain = rule.domain.trim_start_matches("*.");
    let outbound = if rule.action == "proxy" {
      "PROXY"
    } else {
      "DIRECT"
    };
    lines.push(format!("  - DOMAIN-SUFFIX,{domain},{outbound}"));
  }
  if lines.is_empty() {
    return String::new();
  }
  lines.join("\n")
}

pub fn build_dns_policy(rules: &[UserRule]) -> String {
  let mut patterns = BTreeSet::new();
  for rule in rules {
    if rule.action != "direct" {
      continue;
    }
    let domain = rule.domain.trim_start_matches("*.");
    patterns.insert(format!("'+.{domain}'"));
    if let Some(parent) = parent_suffix(domain) {
      patterns.insert(format!("'+.{parent}'"));
    }
  }
  if patterns.is_empty() {
    return String::new();
  }
  let mut block = String::new();
  for pattern in patterns {
    block.push_str(&format!("    {pattern}: [system]\n"));
  }
  block
}

pub fn build_dns_hosts(rules: &[UserRule]) -> String {
  let mut lines = Vec::new();
  for rule in rules {
    if rule.action != "direct" {
      continue;
    }
    let domain = rule.domain.trim_start_matches("*.");
    if let Some(ip) = resolve_system_ipv4(domain) {
      lines.push(format!("    '{domain}': {ip}"));
    }
  }
  if lines.is_empty() {
    return String::new();
  }
  format!("  hosts:\n{}\n", lines.join("\n"))
}

pub fn build_sniffer_skip_domains(rules: &[UserRule]) -> String {
  let mut patterns = BTreeSet::new();
  for rule in rules {
    if rule.action != "direct" {
      continue;
    }
    let domain = rule.domain.trim_start_matches("*.");
    patterns.insert(format!("    - '+.{}'", domain));
    if let Some(parent) = parent_suffix(domain) {
      patterns.insert(format!("    - '+.{}'", parent));
    }
  }
  if patterns.is_empty() {
    return String::new();
  }
  format!("  skip-domain:\n{}\n", patterns.into_iter().collect::<Vec<_>>().join("\n"))
}

pub fn ensure_gfw_rules() {
  let path = rules_dir().join("gfw.txt");
  if path.exists() {
    return;
  }
  let _ = fs::write(&path, b"placeholder.invalid\n");
  std::thread::spawn(move || {
    let url = "https://cdn.jsdelivr.net/gh/Loyalsoldier/clash-rules@release/gfw.txt";
    if let Ok(response) = reqwest::blocking::Client::builder()
      .timeout(std::time::Duration::from_secs(10))
      .build()
      .and_then(|client| client.get(url).send())
    {
      if let Ok(bytes) = response.bytes() {
        let _ = fs::write(path, bytes);
      }
    }
  });
}

pub fn ensure_default_rules() -> Result<(), String> {
  ensure_gfw_rules();
  let rules_path = rules_dir().join("user-rules.json");
  if !rules_path.exists() {
    save_user_rules(&[])?;
  }
  Ok(())
}
