use crate::config::{self, AppSettings, MIXED_PORT};
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn dev_binary() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries/mihomo")
}

fn bundled_binary() -> Option<PathBuf> {
  let exe = std::env::current_exe().ok()?;
  let dir = exe.parent()?;
  for name in ["mihomo", "mihomo-x86_64-apple-darwin", "mihomo-aarch64-apple-darwin"] {
    let path = dir.join(name);
    if path.exists() {
      return Some(path);
    }
  }
  dir.parent()
    .and_then(|macos| macos.parent())
    .map(|resources| resources.join("binaries/mihomo"))
    .filter(|path| path.exists())
}

pub fn mihomo_binary() -> Result<PathBuf, String> {
  let dev = dev_binary();
  if dev.exists() {
    return Ok(dev);
  }
  bundled_binary().ok_or_else(|| {
    "找不到 mihomo 核心，请在项目目录运行: npm run setup".into()
  })
}

pub fn write_mihomo_config(settings: &AppSettings) -> Result<(), String> {
  config::ensure_default_rules()?;
  let rules = config::load_user_rules();
  let user_rules = config::build_inline_rules(&rules);
  let dns_policy = config::build_dns_policy(&rules);
  let dns_hosts = config::build_dns_hosts(&rules);
  let sniffer_skip = config::build_sniffer_skip_domains(&rules);

  let proxy_type = match settings.upstream.proxy_type {
    config::ProxyType::Http => "http",
    config::ProxyType::Socks5 => "socks5",
  };

  let rules_dir = config::rules_dir();
  let gfw_cache = rules_dir.join("gfw-remote.txt");

  let yaml = format!(
    r#"mixed-port: {mixed_port}
allow-lan: false
mode: rule
log-level: warning
external-controller: 127.0.0.1:{api_port}
secret: {secret}
ipv6: false

sniffer:
  enable: true
  force-dns-mapping: true
  parse-pure-ip: true
  override-destination: true
{sniffer_skip}  sniff:
    TLS:
      ports: [443, 8443]
    HTTP:
      ports: [80, 8080-8880]

dns:
  enable: true
  listen: 0.0.0.0:0
  enhanced-mode: redir-host
  default-nameserver:
    - 223.5.5.5
    - 119.29.29.29
  nameserver:
    - https://doh.pub/dns-query
    - https://dns.alidns.com/dns-query
  direct-nameserver:
    - system
  direct-nameserver-follow-policy: true
  proxy-server-nameserver:
    - https://dns.google/dns-query
    - https://cloudflare-dns.com/dns-query
    - 8.8.8.8
  nameserver-policy:
    '+.google.com': [https://dns.google/dns-query, 8.8.8.8]
    '+.google.com.hk': [https://dns.google/dns-query, 8.8.8.8]
    '+.google.cn': [https://dns.google/dns-query, 8.8.8.8]
{dns_policy}{dns_hosts}proxies:
  - name: UPSTREAM
    type: {proxy_type}
    server: {server}
    port: {port}

proxy-groups:
  - name: PROXY
    type: select
    proxies:
      - UPSTREAM

rule-providers:
  gfwlist:
    type: http
    behavior: domain
    format: yaml
    url: https://cdn.jsdelivr.net/gh/Loyalsoldier/clash-rules@release/gfw.txt
    path: {gfw_cache}
    interval: 86400

rules:
{user_rules}{google_rules}
  - RULE-SET,gfwlist,PROXY
  - GEOIP,CN,DIRECT,no-resolve
  - MATCH,DIRECT
"#,
    mixed_port = MIXED_PORT,
    api_port = config::API_PORT,
    secret = config::API_SECRET,
    dns_policy = dns_policy,
    dns_hosts = dns_hosts,
    sniffer_skip = sniffer_skip,
    proxy_type = proxy_type,
    server = settings.upstream.host,
    port = settings.upstream.port,
    gfw_cache = gfw_cache.display(),
    user_rules = if user_rules.is_empty() {
      String::new()
    } else {
      format!("{user_rules}\n")
    },
    google_rules = r#"  - DOMAIN-SUFFIX,google.com,PROXY
  - DOMAIN-SUFFIX,google.com.hk,PROXY
  - DOMAIN-SUFFIX,google.cn,PROXY
  - DOMAIN-SUFFIX,googleapis.com,PROXY
  - DOMAIN-SUFFIX,gstatic.com,PROXY
  - DOMAIN-SUFFIX,ggpht.com,PROXY
  - DOMAIN-SUFFIX,googleusercontent.com,PROXY
"#,
  );

  fs::write(config::config_path(), yaml).map_err(|e| e.to_string())
}

fn cleanup_stale_mihomo() {
  let config = config::config_path().to_string_lossy().to_string();
  let output = Command::new("pgrep").arg("-lf").arg("mihomo").output();
  let Ok(output) = output else { return };
  let text = String::from_utf8_lossy(&output.stdout);
  for line in text.lines() {
    if line.contains(&config) {
      if let Some(pid) = line.split_whitespace().next() {
        let _ = Command::new("kill").arg(pid).status();
      }
    }
  }
}

pub fn spawn_mihomo(settings: &AppSettings) -> Result<Child, String> {
  cleanup_stale_mihomo();
  write_mihomo_config(settings)?;
  let binary = mihomo_binary()?;
  let work_dir = config::data_dir();
  Command::new(binary)
    .arg("-f")
    .arg(config::config_path())
    .arg("-d")
    .arg(work_dir)
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .map_err(|e| format!("启动 mihomo 失败: {e}"))
}

pub fn wait_until_ready() {
  let url = format!("http://127.0.0.1:{}/", config::API_PORT);
  let client = reqwest::blocking::Client::builder()
    .timeout(Duration::from_millis(200))
    .build();
  let Ok(client) = client else { return };
  let deadline = Instant::now() + Duration::from_secs(2);
  while Instant::now() < deadline {
    if client
      .get(&url)
      .header("Authorization", format!("Bearer {}", config::API_SECRET))
      .send()
      .map(|response| response.status().is_success())
      .unwrap_or(false)
    {
      return;
    }
    std::thread::sleep(Duration::from_millis(50));
  }
}

pub fn stop_child(child: &mut Child) {
  let _ = child.kill();
  let _ = child.wait();
}

pub fn reload_running_config(settings: &AppSettings) -> Result<(), String> {
  write_mihomo_config(settings)?;
  let path = config::config_path();
  let client = reqwest::blocking::Client::new();
  let url = format!(
    "http://127.0.0.1:{}/configs?force=true",
    config::API_PORT
  );
  let body = serde_json::json!({ "path": path.to_string_lossy() });
  let response = client
    .put(url)
    .header("Authorization", format!("Bearer {}", config::API_SECRET))
    .json(&body)
    .send()
    .map_err(|e| format!("重载配置失败: {e}"))?;
  if response.status().is_success() {
    return Ok(());
  }
  Err(format!("重载配置失败: HTTP {}", response.status()))
}
