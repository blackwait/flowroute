import { useCallback, useEffect, useState } from 'react';
import { api, AppSettings, ConnectionItem, StatusResponse, UserRule } from './api';

function actionLabel(chains: string[]) {
  const joined = chains.join(' ');
  if (joined.includes('DIRECT')) return '直连';
  if (joined.includes('UPSTREAM') || joined.includes('PROXY')) return '代理';
  return joined || '未知';
}

export default function App() {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [connections, setConnections] = useState<ConnectionItem[]>([]);
  const [recentDomains, setRecentDomains] = useState<string[]>([]);
  const [rules, setRules] = useState<UserRule[]>([]);
  const [currentPage, setCurrentPage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [toggleBusy, setToggleBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [draft, setDraft] = useState<AppSettings | null>(null);

  const refreshLight = useCallback(async () => {
    const [nextStatus, nextRules] = await Promise.all([api.getStatus(), api.getUserRules()]);
    setStatus(nextStatus);
    setRules(nextRules);
    if (!nextStatus.running) {
      setConnections([]);
    }
  }, []);

  const refreshHeavy = useCallback(async () => {
    try {
      const nextStatus = await api.getStatus();
      if (nextStatus.running) {
        const [items, page] = await Promise.all([
          api.getConnections().catch(() => []),
          api.getCurrentPage().catch(() => null),
        ]);
        setConnections(items);
        if (items.length > 0) {
          setRecentDomains((prev) => {
            const next = [...items.map((item) => item.domain), ...prev];
            return [...new Set(next)].slice(0, 30);
          });
        }
        setCurrentPage(page);
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
    refresh();
    const timer = window.setInterval(refresh, 3000);
    return () => window.clearInterval(timer);
  }, [refresh]);

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
      setStatus({ ...status, running: turningOn, system_proxy: turningOn });
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

  const ruleAction = (domain: string) => rules.find((item) => item.domain === domain)?.action;

  return (
    <div className="app">
      <header className="hero">
        <div>
          <p className="eyebrow">FlowRoute</p>
          <h1>一键分流，不用浏览器扩展</h1>
          <p className="subtitle">内置 GFW 规则，遇到新网站点一下就能决定走代理还是直连。</p>
        </div>
        <button className="ghost" onClick={openSettings}>设置</button>
      </header>

      <section className="card power-card">
        <div>
          <div className="status-line">
            <span className={`dot ${status?.running ? 'on' : 'off'}`} />
            <strong>{status?.running ? '分流已开启' : '分流已关闭'}</strong>
          </div>
          <p className="hint">
            {status?.running
              ? `系统代理 127.0.0.1:${status.mixed_port}，浏览器会自动生效`
              : '开启后会设置系统代理，并加载内置规则'}
          </p>
        </div>
        <button
          className={`power ${status?.running ? 'stop' : 'start'}`}
          disabled={toggleBusy}
          onClick={toggleRouting}
        >
          {toggleBusy ? '处理中…' : status?.running ? '关闭' : '开启分流'}
        </button>
      </section>

      <section className="card">
        <div className="section-head">
          <h2>当前网页</h2>
          <button className="ghost small" onClick={() => withBusy(refresh)}>刷新</button>
        </div>
        {currentPage ? (
          <>
            <p className="domain">{currentPage}</p>
            <div className="actions">
              <button disabled={busy} onClick={() => applyRule(currentPage, 'proxy')}>走代理</button>
              <button className="secondary" disabled={busy} onClick={() => applyRule(currentPage, 'direct')}>直连</button>
              {ruleAction(currentPage) && (
                <span className="badge">{ruleAction(currentPage) === 'proxy' ? '已设代理' : '已设直连'}</span>
              )}
            </div>
          </>
        ) : (
          <p className="hint">未检测到浏览器标签页。请先在 Edge / Chrome 打开网页，再点刷新。</p>
        )}
      </section>

      <section className="card">
        <div className="section-head">
          <h2>最近访问</h2>
          <span className="hint">{status?.running ? '浏览时会自动记录' : '开启分流后显示'}</span>
        </div>
        {recentDomains.length === 0 ? (
          <p className="hint">暂无记录。开启分流后浏览网页，这里会出现最近访问的域名。</p>
        ) : (
          <ul className="list scrollable">
            {recentDomains.map((domain) => {
              const live = connections.find((item) => item.domain === domain);
              return (
                <li key={domain}>
                  <div>
                    <strong>{domain}</strong>
                    <span>{live ? actionLabel(live.chains) : '最近访问'}</span>
                  </div>
                  <div className="actions compact">
                    <button disabled={busy} onClick={() => applyRule(domain, 'proxy')}>代理</button>
                    <button className="secondary" disabled={busy} onClick={() => applyRule(domain, 'direct')}>直连</button>
                  </div>
                </li>
              );
            })}
          </ul>
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
