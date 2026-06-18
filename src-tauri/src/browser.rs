use crate::clash_api::normalize_domain;
use std::process::Command;

const BROWSERS: &[(&str, &str)] = &[
  ("Microsoft Edge", r#"tell application "Microsoft Edge" to if (count of windows) > 0 then return URL of active tab of front window"#),
  ("Google Chrome", r#"tell application "Google Chrome" to if (count of windows) > 0 then return URL of active tab of front window"#),
  ("Safari", r#"tell application "Safari" to if (count of windows) > 0 then return URL of current tab of front window"#),
  ("Arc", r#"tell application "Arc" to if (count of windows) > 0 then return URL of active tab of front window"#),
];

pub fn current_browser_url() -> Result<Option<String>, String> {
  #[cfg(not(target_os = "macos"))]
  {
    return Ok(None);
  }

  #[cfg(target_os = "macos")]
  {
    for (name, script) in BROWSERS {
      let output = Command::new("osascript")
        .arg("-e")
        .arg(*script)
        .output()
        .map_err(|e| format!("读取 {name} 失败: {e}"))?;
      if !output.status.success() {
        continue;
      }
      let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
      if !url.is_empty() {
        return Ok(normalize_domain(&url));
      }
    }
    Ok(None)
  }
}
