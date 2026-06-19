use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::Duration;

use crate::dns_check::resolve_system_ipv4;

pub const MIXED_PORT: u16 = 17890;
pub const API_PORT: u16 = 19090;
pub const API_SECRET: &str = "flowroute-local";
pub const AUTO_DETECT_UPSTREAM_PORTS: [u16; 2] = [7897, 7890];

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
      port: 7897,
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
      if let Ok(mut settings) = serde_json::from_str::<AppSettings>(&text) {
        if !is_upstream_reachable(&settings.upstream) {
          if let Some(port) = detect_upstream_port(&settings.upstream.host) {
            settings.upstream.port = port;
            let _ = save_settings(&settings);
          }
        }
        return settings;
      }
    }
  }
  let mut settings = AppSettings::default();
  if let Some(port) = detect_upstream_port(&settings.upstream.host) {
    settings.upstream.port = port;
  }
  settings
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
  let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
  fs::write(settings_path(), text).map_err(|e| e.to_string())
}

pub fn detect_upstream_port(host: &str) -> Option<u16> {
  AUTO_DETECT_UPSTREAM_PORTS
    .iter()
    .copied()
    .find(|port| is_host_port_reachable(host, *port))
}

pub fn is_upstream_reachable(upstream: &UpstreamProxy) -> bool {
  is_host_port_reachable(&upstream.host, upstream.port)
}

fn is_host_port_reachable(host: &str, port: u16) -> bool {
  let address = format!("{}:{}", host.trim(), port);
  let timeout = Duration::from_millis(220);
  match address.to_socket_addrs() {
    Ok(addresses) => addresses
      .into_iter()
      .any(|addr| TcpStream::connect_timeout(&addr, timeout).is_ok()),
    Err(_) => false,
  }
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

pub fn build_dns_hosts(rules: &[UserRule]) -> Vec<String> {
  let mut lines = vec!["    'localhost': 127.0.0.1".to_string()];
  for rule in rules {
    if rule.action != "direct" {
      continue;
    }
    let domain = rule.domain.trim_start_matches("*.");
    if let Some(ip) = resolve_system_ipv4(domain) {
      lines.push(format!("    '{domain}': {ip}"));
    }
  }
  lines
}

pub fn format_dns_hosts(lines: &[String]) -> String {
  if lines.is_empty() {
    return String::new();
  }
  format!("  hosts:\n{}\n", lines.join("\n"))
}

pub fn build_sniffer_skip_domains(rules: &[UserRule]) -> String {
  let mut patterns = BTreeSet::new();
  for domain in local_direct_domains() {
    patterns.insert(format!("    - '{}'", domain));
  }
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

pub fn build_local_direct_dns_policy() -> String {
  let mut block = String::new();
  for domain in local_direct_domains() {
    block.push_str(&format!("    '{}': [system]\n", domain));
  }
  block
}

pub fn build_local_direct_rules() -> String {
  r#"  - DOMAIN,localhost,DIRECT
  - DOMAIN-SUFFIX,local,DIRECT
  - IP-CIDR,127.0.0.0/8,DIRECT,no-resolve
  - IP-CIDR,10.0.0.0/8,DIRECT,no-resolve
  - IP-CIDR,172.16.0.0/12,DIRECT,no-resolve
  - IP-CIDR,192.168.0.0/16,DIRECT,no-resolve
  - IP-CIDR,169.254.0.0/16,DIRECT,no-resolve
  - IP-CIDR,100.64.0.0/10,DIRECT,no-resolve
  - IP-CIDR6,::1/128,DIRECT,no-resolve
  - IP-CIDR6,fc00::/7,DIRECT,no-resolve
  - IP-CIDR6,fe80::/10,DIRECT,no-resolve
"#
  .into()
}

fn local_direct_domains() -> &'static [&'static str] {
  &["localhost", "+.local", "*.local"]
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
