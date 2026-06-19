# Browser-Only Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Change FlowRoute from system proxy mode to browser-process proxy mode so only Edge and Chrome launched by FlowRoute use the local mihomo routing.

**Architecture:** Keep the existing mihomo core and domain rule pipeline, but remove macOS system proxy toggling. Add a small browser launcher in Rust that starts Edge or Chrome with `--proxy-server=127.0.0.1:<mixed_port>` and track the selected browser in app state for the UI.

**Tech Stack:** Tauri 2, Rust, React, TypeScript, Vite

---

### Task 1: Replace system proxy state with browser routing state

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Delete: `src-tauri/src/sysproxy.rs`

- [ ] Define app state around running core and selected browser instead of system proxy flags.
- [ ] Remove start/stop calls that mutate macOS system proxy.
- [ ] Ensure app exit only stops mihomo child process.

### Task 2: Add Edge/Chrome launcher and current-page detection boundaries

**Files:**
- Modify: `src-tauri/src/browser.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] Add serializable browser enum and launcher helpers for Google Chrome / Microsoft Edge.
- [ ] Expose a Tauri command to open a proxied browser process with the local mixed port.
- [ ] Restrict current-page detection to Edge / Chrome only.

### Task 3: Update frontend API and UI wording

**Files:**
- Modify: `src/api.ts`
- Modify: `src/App.tsx`

- [ ] Replace `system_proxy` status fields with browser-selection status.
- [ ] Add browser selector plus “open browser” action.
- [ ] Update copy so the app clearly says routing only affects browsers opened from FlowRoute.

### Task 4: Verify build

**Files:**
- Modify: none

- [ ] Run `npm run build`.
- [ ] Run `cargo check` in `src-tauri`.
- [ ] Review outputs and fix any compile issues before reporting completion.
