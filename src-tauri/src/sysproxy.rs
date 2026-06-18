use crate::config::MIXED_PORT;
use std::process::Command;
use std::sync::OnceLock;

static SERVICES_CACHE: OnceLock<Vec<String>> = OnceLock::new();

fn all_network_services() -> Result<Vec<String>, String> {
  if let Some(cached) = SERVICES_CACHE.get() {
    return Ok(cached.clone());
  }
  let output = Command::new("networksetup")
    .arg("-listallnetworkservices")
    .output()
    .map_err(|e| format!("networksetup 不可用: {e}"))?;
  let text = String::from_utf8_lossy(&output.stdout);
  let mut services = Vec::new();
  for line in text.lines().skip(1) {
    let name = line.trim();
    if name.is_empty() || name.starts_with('*') || name.contains("Serial Port") {
      continue;
    }
    services.push(name.to_string());
  }
  if services.is_empty() {
    return Err("未找到可用网络服务".into());
  }
  let _ = SERVICES_CACHE.set(services.clone());
  Ok(services)
}

fn default_interface() -> Option<String> {
  let output = Command::new("route").args(["-n", "get", "default"]).output().ok()?;
  for line in String::from_utf8_lossy(&output.stdout).lines() {
    let line = line.trim();
    if let Some(iface) = line.strip_prefix("interface:") {
      let iface = iface.trim();
      if !iface.is_empty() {
        return Some(iface.to_string());
      }
    }
  }
  None
}

fn service_for_interface(iface: &str) -> Option<String> {
  let output = Command::new("networksetup")
    .arg("-listnetworkserviceorder")
    .output()
    .ok()?;
  let mut current_service: Option<String> = None;
  for line in String::from_utf8_lossy(&output.stdout).lines() {
    let line = line.trim();
    if line.starts_with('(') && line.contains(')') {
      current_service = line.split(')').nth(1).map(|name| name.trim().to_string());
      continue;
    }
    if line.contains("Device:") && line.contains(iface) {
      return current_service;
    }
  }
  None
}

fn primary_network_service() -> Result<String, String> {
  if let Some(iface) = default_interface() {
    if let Some(service) = service_for_interface(&iface) {
      return Ok(service);
    }
  }
  all_network_services()?
    .into_iter()
    .next()
    .ok_or_else(|| "未找到主网络服务".into())
}

fn set_proxy_for_service(service: &str, enabled: bool) -> Result<(), String> {
  let port = MIXED_PORT.to_string();
  if enabled {
    for flag in ["-setwebproxy", "-setsecurewebproxy"] {
      let status = Command::new("networksetup")
        .arg(flag)
        .arg(service)
        .arg("127.0.0.1")
        .arg(&port)
        .status()
        .map_err(|e| format!("设置 {service} 代理失败: {e}"))?;
      if !status.success() {
        return Err(format!("设置 {service} 代理失败"));
      }
    }
    for flag in ["-setwebproxystate", "-setsecurewebproxystate"] {
      Command::new("networksetup")
        .args([flag, service, "on"])
        .status()
        .map_err(|e| e.to_string())?;
    }
  } else {
    for flag in ["-setwebproxystate", "-setsecurewebproxystate"] {
      Command::new("networksetup")
        .args([flag, service, "off"])
        .status()
        .map_err(|e| e.to_string())?;
    }
  }
  Ok(())
}

pub fn enable_system_proxy() -> Result<(), String> {
  set_proxy_for_service(&primary_network_service()?, true)
}

pub fn disable_system_proxy() -> Result<(), String> {
  let _ = set_proxy_for_service(&primary_network_service()?, false);
  Ok(())
}
