import { invoke } from '@tauri-apps/api/core';

export type ProxyType = 'http' | 'socks5';

export interface UpstreamProxy {
  type: ProxyType;
  host: string;
  port: number;
}

export interface AppSettings {
  upstream: UpstreamProxy;
}

export type BrowserKind = 'edge' | 'chrome';

export interface StatusResponse {
  running: boolean;
  selected_browser: BrowserKind;
  supported_browsers: BrowserKind[];
  mixed_port: number;
  settings: AppSettings;
}

export interface ConnectionItem {
  domain: string;
  host: string;
  network: string;
  rule: string;
  chains: string[];
}

export interface UserRule {
  domain: string;
  action: 'proxy' | 'direct';
}

export interface AddRuleResult {
  rules: UserRule[];
  warning?: string | null;
}

export const api = {
  getStatus: () => invoke<StatusResponse>('get_status'),
  saveSettings: (settings: AppSettings) => invoke<void>('save_app_settings', { settings }),
  start: () => invoke<StatusResponse>('start_routing'),
  stop: () => invoke<StatusResponse>('stop_routing'),
  setBrowser: (browser: BrowserKind) => invoke<StatusResponse>('set_browser', { browser }),
  openBrowser: (browser?: BrowserKind, url?: string) => invoke<StatusResponse>('open_browser', { browser, url }),
  getConnections: () => invoke<ConnectionItem[]>('get_connections'),
  getUserRules: () => invoke<UserRule[]>('get_user_rules'),
  addRule: (domain: string, action: 'proxy' | 'direct') =>
    invoke<AddRuleResult>('add_rule', { domain, action }),
  deleteRule: (domain: string) => invoke<UserRule[]>('delete_rule', { domain }),
  getCurrentPage: () => invoke<string | null>('get_current_page'),
};
