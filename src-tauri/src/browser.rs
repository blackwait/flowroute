use crate::clash_api::normalize_domain;
use crate::config::{self, MIXED_PORT};
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserKind {
  Edge,
  Chrome,
}

impl BrowserKind {
  pub fn label(self) -> &'static str {
    match self {
      Self::Edge => "Microsoft Edge",
      Self::Chrome => "Google Chrome",
    }
  }

  fn bundle_id(self) -> &'static str {
    match self {
      Self::Edge => "com.microsoft.edgemac",
      Self::Chrome => "com.google.Chrome",
    }
  }

  fn current_tab_script(self) -> String {
    format!(
      r#"tell application "{}" to if (count of windows) > 0 then return URL of active tab of front window"#,
      self.label()
    )
  }

  fn profile_dir_name(self) -> &'static str {
    match self {
      Self::Edge => "edge",
      Self::Chrome => "chrome",
    }
  }
}

const SUPPORTED_BROWSERS: &[BrowserKind] = &[BrowserKind::Edge, BrowserKind::Chrome];

pub fn supported_browsers() -> Vec<BrowserKind> {
  SUPPORTED_BROWSERS.to_vec()
}

pub fn current_browser_url() -> Result<Option<String>, String> {
  #[cfg(not(target_os = "macos"))]
  {
    return Ok(None);
  }

  #[cfg(target_os = "macos")]
  {
    for browser in SUPPORTED_BROWSERS {
      let output = Command::new("osascript")
        .arg("-e")
        .arg(browser.current_tab_script())
        .output()
        .map_err(|e| format!("读取 {} 失败: {e}", browser.label()))?;
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

pub fn launch_browser(browser: BrowserKind, target: Option<&str>) -> Result<(), String> {
  #[cfg(not(target_os = "macos"))]
  {
    let _ = (browser, target);
    Err("当前仅支持 macOS".into())
  }

  #[cfg(target_os = "macos")]
  {
    let profile_dir = config::data_dir()
      .join("browser-profiles")
      .join(browser.profile_dir_name());
    fs::create_dir_all(&profile_dir).map_err(|e| format!("创建受控浏览器配置目录失败: {e}"))?;

    let mut command = Command::new("open");
    command
      .arg("-n")
      .arg("-b")
      .arg(browser.bundle_id())
      .arg("--args")
      .arg(format!("--proxy-server=http://127.0.0.1:{MIXED_PORT}"))
      .arg(format!("--user-data-dir={}", profile_dir.display()))
      .arg("--no-first-run")
      .arg("--new-window");

    if let Some(url) = target {
      let trimmed = url.trim();
      if !trimmed.is_empty() {
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
          command.arg(trimmed);
        } else {
          command.arg(format!("https://{trimmed}"));
        }
      }
    }

    command
      .spawn()
      .map_err(|e| format!("启动 {} 失败: {e}", browser.label()))?;
    Ok(())
  }
}
