use std::process::Command;

pub fn domain_resolvable(domain: &str) -> bool {
  resolve_system_ipv4(domain).is_some()
}

pub fn resolve_system_ipv4(domain: &str) -> Option<String> {
  let output = Command::new("dscacheutil")
    .args(["-q", "host", "-a", "name", domain])
    .output()
    .ok()?;
  let text = String::from_utf8_lossy(&output.stdout);
  for line in text.lines() {
    let line = line.trim();
    if let Some(ip) = line.strip_prefix("ip_address:") {
      let ip = ip.trim();
      if !ip.is_empty() {
        return Some(ip.to_string());
      }
    }
  }
  None
}
