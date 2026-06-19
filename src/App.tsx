import { useCallback, useEffect, useState } from 'react';
import { api, AppSettings, BrowserKind, StatusResponse, UserRule } from './api';

function browserLabel(browser: BrowserKind) {
  return browser === 'edge' ? 'Microsoft Edge' : 'Google Chrome';
}

export default function App() {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [rules, setRules] = useState<UserRule[]>([]);
  const [currentPage, setCurrentPage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [toggleBusy, setToggleBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [draft, setDraft] = useState<AppSettings | null>(null);
  const [bootstrapping, setBootstrapping] = useState(true);

  const refreshLight = useCallback(async () => {
    const [nextStatus, nextRules] = await Promise.all([api.getStatus(), api.getUserRules()]);
    setStatus(nextStatus);
    setRules(nextRules);
    if (!nextStatus.running) {
      setCurrentPage(null);
    }
  }, []);

  const refreshHeavy = useCallback(async () => {
    try {
      const nextStatus = await api.getStatus();
      if (nextStatus.running) {
        const page = await api.getCurrentPage().catch(() => null);
        setCurrentPage(page);
      } else {
        setCurrentPage(null);
      }
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  const refresh = useCallback(async () => {
    try {
      await refreshLight();
      await refreshHeavy();
    } catch (error) {
      setMessage(String(error));
    }
  }, [refreshLight, refreshHeavy]);

  useEffect(() => {
    let disposed = false;

    const bootstrap = async () => {
      try {
        const [initialStatus, nextRules] = await Promise.all([api.getStatus(), api.getUserRules()]);
        if (disposed) return;
        setRules(nextRules);

        let nextStatus = initialStatus;
        if (initialStatus.selected_browser !== 'edge') {
          nextStatus = await api.setBrowser('edge');
          if (disposed) return;
        }
        if (!nextStatus.running) {
          nextStatus = await api.start();
          if (disposed) return;
        }

        setStatus(nextStatus);
        await refreshHeavy();
      } catch (error) {
        if (!disposed) {
          setMessage(String(error));
        }
      } finally {
        if (!disposed) {
          setBootstrapping(false);
        }
      }
    };

    bootstrap();
    const timer = window.setInterval(() => {
      void refresh();
    }, 3000);
    return () => window.clearInterval(timer);
  }, [refresh, refreshHeavy]);

  const withBusy = async (task: () => Promise<void>) => {
    setBusy(true);
    setMessage('');
    try {
      await task();
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };

  const toggleRouting = async () => {
    if (toggleBusy) return;
    setToggleBusy(true);
    setMessage('');
    const turningOn = !status?.running;
    if (status) {
      setStatus({ ...status, running: turningOn });
    }
    try {
      const next = turningOn ? await api.start() : await api.stop();
      setStatus(next);
      void refreshHeavy();
    } catch (error) {
      await refreshLight();
      setMessage(String(error));
    } finally {
      setToggleBusy(false);
    }
  };

  const applyRule = (domain: string, action: 'proxy' | 'direct') =>
    withBusy(async () => {
      const result = await api.addRule(domain, action);
      setRules(result.rules);
      if (result.warning) {
        setMessage(result.warning);
      } else {
        setMessage(`已设置 ${domain} → ${action === 'proxy' ? '走代理' : '直连'}，刷新页面后生效`);
      }
    });

  const openSettings = () => {
    if (status) {
      setDraft(structuredClone(status.settings));
      setSettingsOpen(true);
    }
  };

  const saveSettings = () =>
    withBusy(async () => {
      if (!draft) return;
      await api.saveSettings(draft);
      setSettingsOpen(false);
      setMessage('上游代理已保存，重新开启分流后生效');
    });

  const changeBrowser = async (browser: BrowserKind) => {
    try {
      const next = await api.setBrowser(browser);
      setStatus(next);
      setMessage(`已切换浏览器为 ${browserLabel(browser)}`);
    } catch (error) {
      setMessage(String(error));
    }
  };

  const openBrowser = async () => {
    try {
      const next = await api.openBrowser(status?.selected_browser, currentPage ?? undefined);
      setStatus(next);
      setMessage(`已通过 ${browserLabel(next.selected_browser)} 打开受控浏览器`);
    } catch (error) {
      setMessage(String(error));
    }
  };

  const ruleAction = (domain: string) => rules.find((item) => item.domain === domain)?.action;
  const currentStatusText = status?.running ? '分流在线' : bootstrapping ? '正在启动' : '分流关闭';
  const primaryHint = status?.running
    ? `127.0.0.1:${status.mixed_port} / ${browserLabel(status.selected_browser)}`
    : '不改系统代理，不影响 git 和其它应用';

  return (
    <div className="app">
      <header className="hero">
        <div className="hero-copy">
          <div className="hero-topline">
            <p className="eyebrow">FlowRoute</p>
            <span className={`hero-pill ${status?.running ? 'live' : 'idle'}`}>{currentStatusText}</span>
          </div>
          <h1>FlowRoute</h1>
          <p className="subtitle">启动即分流，默认 Edge。</p>
        </div>
        <button className="ghost hero-settings" onClick={openSettings}>设置</button>
      </header>

      <section className="card power-card accent-card">
        <div className="power-copy">
          <div className="status-line">
            <span className={`dot ${status?.running ? 'on' : 'off'}`} />
            <strong>{bootstrapping ? '启动中…' : status?.running ? '分流已开启' : '分流已关闭'}</strong>
          </div>
          <p className="subtitle power-subtitle">{primaryHint}</p>
          <div className="metric-row">
            <div className="metric-chip">
              <span>默认浏览器</span>
              <strong>{browserLabel(status?.selected_browser ?? 'edge')}</strong>
            </div>
            <div className="metric-chip">
              <span>代理端口</span>
              <strong>{status?.mixed_port ?? 17890}</strong>
            </div>
          </div>
        </div>
        <div className="stack-actions">
          <button
            className={`power ${status?.running ? 'stop' : 'start'}`}
            disabled={toggleBusy || bootstrapping}
            onClick={toggleRouting}
          >
            {toggleBusy ? '处理中…' : status?.running ? '关闭分流' : '开启分流'}
          </button>
        </div>
      </section>

      <section className="card card-grid">
        <div className="section-head">
          <h2>受控浏览器</h2>
          <span className="section-tag">Edge / Chrome</span>
        </div>
        <div className="control-panel">
          <div className="segment-control" role="tablist" aria-label="选择受控浏览器">
            {(status?.supported_browsers ?? ['edge', 'chrome']).map((browser) => (
              <button
                key={browser}
                type="button"
                className={`segment-option ${status?.selected_browser === browser ? 'active' : ''}`}
                onClick={() => changeBrowser(browser)}
              >
                {browser === 'edge' ? 'Edge' : 'Chrome'}
              </button>
            ))}
          </div>
          <div className="actions spacious">
            <button className="primary-wide" disabled={!status?.running || bootstrapping} onClick={openBrowser}>
              打开受控浏览器
            </button>
            <button className="secondary slim-button" disabled={bootstrapping} onClick={() => void refresh()}>
              刷新状态
            </button>
          </div>
        </div>
      </section>

      <section className="card">
        <div className="section-head">
          <h2>当前网页</h2>
          <button className="ghost small" onClick={() => withBusy(refresh)}>刷新</button>
        </div>
        {currentPage ? (
          <>
            <p className="domain">{currentPage}</p>
            <div className="actions spacious">
              <button className="primary-wide" disabled={busy} onClick={() => applyRule(currentPage, 'proxy')}>走代理</button>
              <button className="secondary" disabled={busy} onClick={() => applyRule(currentPage, 'direct')}>直连</button>
              {ruleAction(currentPage) && (
                <span className="badge">{ruleAction(currentPage) === 'proxy' ? '已设代理' : '已设直连'}</span>
              )}
            </div>
          </>
        ) : (
          <p className="hint">未检测到 Edge / Chrome 当前标签页。请先用上面的按钮打开浏览器，再点刷新。</p>
        )}
      </section>

      <section className="card">
        <div className="section-head">
          <h2>我的规则</h2>
          <span className="hint">{rules.length} 条</span>
        </div>
        {rules.length === 0 ? (
          <p className="hint">还没有自定义规则，遇到新网站时在上面点一下即可。</p>
        ) : (
          <ul className="list rules scrollable">
            {rules.map((rule) => (
              <li key={rule.domain}>
                <div>
                  <strong>{rule.domain}</strong>
                  <span>{rule.action === 'proxy' ? '走代理' : '直连'}</span>
                </div>
                <button
                  className="ghost small"
                  disabled={busy}
                  onClick={() => withBusy(async () => setRules(await api.deleteRule(rule.domain)))}
                >
                  删除
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      {message && <p className="toast">{message}</p>}

      {settingsOpen && draft && (
        <div className="modal-backdrop" onClick={() => setSettingsOpen(false)}>
          <div className="modal" onClick={(event) => event.stopPropagation()}>
            <h2>上游代理</h2>
            <p className="hint">默认转发到 127.0.0.1:7890（你的 Clash/V2Ray 端口），可在设置里修改。</p>
            <label>
              协议
              <select
                value={draft.upstream.type}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    upstream: { ...draft.upstream, type: event.target.value as 'http' | 'socks5' },
                  })
                }
              >
                <option value="socks5">SOCKS5</option>
                <option value="http">HTTP</option>
              </select>
            </label>
            <label>
              地址
              <input
                value={draft.upstream.host}
                onChange={(event) =>
                  setDraft({ ...draft, upstream: { ...draft.upstream, host: event.target.value } })
                }
              />
            </label>
            <label>
              端口
              <input
                type="number"
                value={draft.upstream.port}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    upstream: { ...draft.upstream, port: Number(event.target.value) || 7890 },
                  })
                }
              />
            </label>
            <div className="actions">
              <button disabled={busy} onClick={saveSettings}>保存</button>
              <button className="secondary" onClick={() => setSettingsOpen(false)}>取消</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
