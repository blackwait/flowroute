mod browser;
mod clash_api;
mod config;
mod dns_check;
mod mihomo;
mod sysproxy;

use config::{load_settings, save_settings, AppSettings, UserRule, MIXED_PORT};
use clash_api::{fetch_connections, remove_user_rule, upsert_user_rule, AddRuleResult, ConnectionItem};
use mihomo::{reload_running_config, spawn_mihomo, stop_child, wait_until_ready};
use std::process::Child;
use std::sync::Mutex;
use tauri::{Manager, RunEvent, State};

struct EngineState {
  child: Mutex<Option<Child>>,
  proxy_on: Mutex<bool>,
}

#[derive(serde::Serialize)]
struct StatusResponse {
  running: bool,
  system_proxy: bool,
  mixed_port: u16,
  settings: AppSettings,
}

fn build_status(state: &EngineState, running: bool) -> StatusResponse {
  let system_proxy = *state.proxy_on.lock().unwrap();
  StatusResponse {
    running,
    system_proxy,
    mixed_port: MIXED_PORT,
    settings: load_settings(),
  }
}

#[tauri::command]
fn get_status(state: State<'_, EngineState>) -> StatusResponse {
  let running = state.child.lock().unwrap().is_some();
  build_status(&state, running)
}

#[tauri::command]
fn save_app_settings(settings: AppSettings) -> Result<(), String> {
  save_settings(&settings)
}

#[tauri::command]
fn start_routing(state: State<'_, EngineState>) -> Result<StatusResponse, String> {
  {
    let mut guard = state.child.lock().unwrap();
    if guard.is_some() {
      return Ok(build_status(&state, true));
    }
    let settings = load_settings();
    let child = spawn_mihomo(&settings)?;
    *guard = Some(child);
  }
  wait_until_ready();
  sysproxy::enable_system_proxy()?;
  *state.proxy_on.lock().unwrap() = true;
  Ok(build_status(&state, true))
}

#[tauri::command]
fn stop_routing(state: State<'_, EngineState>) -> Result<StatusResponse, String> {
  {
    let mut guard = state.child.lock().unwrap();
    if let Some(mut child) = guard.take() {
      stop_child(&mut child);
    }
  }
  sysproxy::disable_system_proxy()?;
  *state.proxy_on.lock().unwrap() = false;
  Ok(build_status(&state, false))
}

#[tauri::command]
fn get_connections() -> Result<Vec<ConnectionItem>, String> {
  fetch_connections()
}

#[tauri::command]
fn get_user_rules() -> Vec<UserRule> {
  config::load_user_rules()
}

#[tauri::command]
fn add_rule(domain: String, action: String) -> Result<AddRuleResult, String> {
  upsert_user_rule(&domain, &action)
}

#[tauri::command]
fn delete_rule(domain: String) -> Result<Vec<UserRule>, String> {
  remove_user_rule(&domain)
}

#[tauri::command]
fn get_current_page() -> Result<Option<String>, String> {
  browser::current_browser_url()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let _ = config::ensure_default_rules();
  config::ensure_gfw_rules();
  tauri::Builder::default()
    .manage(EngineState {
      child: Mutex::new(None),
      proxy_on: Mutex::new(false),
    })
    .invoke_handler(tauri::generate_handler![
      get_status,
      save_app_settings,
      start_routing,
      stop_routing,
      get_connections,
      get_user_rules,
      add_rule,
      delete_rule,
      get_current_page,
    ])
    .build(tauri::generate_context!())
    .expect("error while building FlowRoute")
    .run(|app_handle, event| {
      if matches!(event, RunEvent::Exit) {
        if let Some(state) = app_handle.try_state::<EngineState>() {
          let mut guard = state.child.lock().unwrap();
          if let Some(mut child) = guard.take() {
            stop_child(&mut child);
          }
        }
        let _ = sysproxy::disable_system_proxy();
      }
    });
}
