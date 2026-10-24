/* Bounded, page-local diagnostics. No host callbacks, network fetches or arbitrary code. */
(() => {
  'use strict';
  const a = __PANDACODE_REVIEW_ARGS__;
  const KEY = '__pandacode_review_v1';
  const safe = (value, max = 240) => String(value ?? '')
    .replace(/-----BEGIN [^-]*PRIVATE KEY-----[\s\S]*/g, '[redacted private key]')
    .replace(/\b(Bearer\s+)[A-Za-z0-9._~+\/-]+=*/gi, '$1[redacted]')
    .replace(/\b((?:api[_-]?key|access[_-]?token|token|password|secret|authorization)\s*[:=]\s*)[^\s,;"'<>]+/gi, '$1[redacted]')
    .replace(/\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|xox[baprs]-[A-Za-z0-9-]{10,})\b/g, '[redacted token]')
    .slice(0, max);
  const url = value => { try { const u = new URL(value, location.href); return /^https?:$/.test(u.protocol) ? u.origin + u.pathname : '[non-http resource]'; } catch { return '[invalid URL]'; } };
  const origin = location.origin;
  const meta = () => ({ schema: 1, ok: true, url: url(location.href), title: safe(document.title), viewport: { width: innerWidth, height: innerHeight } });
  try {
    if (!a || typeof a !== 'object' || a.origin !== origin) throw new Error('Browser origin changed. Reopen the review for the current origin.');
    const supported = ['snapshot', 'audit', 'resources', 'observe_start', 'observe_read', 'observe_stop', 'clear', 'expect_text', 'expect_title', 'expect_url', 'expect_element', 'expect_resource', 'click', 'fill', 'select', 'check', 'focus', 'scroll'];
    if (!supported.includes(a.op)) throw new Error('Unknown browser review operation.');
    for (const k of ['value', 'selector', 'token', 'ref']) if (a[k] !== undefined && (typeof a[k] !== 'string' || a[k].length > 4096 || a[k].includes('\0'))) throw new Error('Invalid browser review input.');
    let state = globalThis[KEY];
    if (!state) {
      state = { refs: new Map(), token: '', stale: true, observer: null, expiry: 0, errors: [], listening: false, timer: 0, error: null, rejection: null };
      Object.defineProperty(globalThis, KEY, { value: state, configurable: true });
    }
    if (!(state.refs instanceof Map) || !Array.isArray(state.errors)) throw new Error('Review state is unavailable. Reload the page.');
    const invalidate = () => { state.stale = true; state.refs.clear(); state.observer?.disconnect(); state.observer = null; };
    const stop = () => {
      if (state.error) removeEventListener('error', state.error, true);
      if (state.rejection) removeEventListener('unhandledrejection', state.rejection);
      clearTimeout(state.timer); state.timer = 0; state.error = null; state.rejection = null; state.listening = false;
    };
    const visible = node => {
      if (!(node instanceof Element) || node.closest('[hidden],[inert],[aria-hidden="true"]')) return false;
      for (let el = node, depth = 0; el && depth < 40; el = el.parentElement, depth++) {
        const style = getComputedStyle(el);
        if (style.display === 'none' || style.visibility === 'hidden' || style.visibility === 'collapse') return false;
      }
      return node.getClientRects().length > 0;
    };
    const privateNode = node => Boolean(node.closest('input,textarea,select,[contenteditable]:not([contenteditable="false"]),script,style,noscript,template'));
    const textOf = (node, limit = 180) => {
      const walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT);
      let text = '', current, scanned = 0;
      while ((current = walker.nextNode()) && text.length < limit && scanned++ < 3000) {
        if (current.parentElement && visible(current.parentElement) && !privateNode(current.parentElement)) text += ' ' + current.textContent;
      }
      return safe(text.replace(/\s+/g, ' ').trim(), limit);
    };
    const nameOf = el => {
      const ids = (el.getAttribute('aria-labelledby') || '').split(/\s+/).filter(Boolean).slice(0, 8);
      const labelled = ids.map(id => document.getElementById(id)).filter(Boolean).map(n => textOf(n)).join(' ');
      const labels = el.labels ? [...el.labels].slice(0, 8).map(n => textOf(n)).join(' ') : '';
      return safe(el.getAttribute('aria-label') || labelled || labels || el.getAttribute('alt') || textOf(el) || el.getAttribute('title') || '', 180);
    };
    const selectorOf = el => {
      if (el.id && document.querySelectorAll('#' + CSS.escape(el.id)).length === 1) return '#' + CSS.escape(el.id);
      const parts = []; let node = el;
      while (node && node !== document.body && parts.length < 16) {
        const tag = node.localName, siblings = node.parentElement ? [...node.parentElement.children].filter(n => n.localName === tag) : [];
        parts.unshift(tag + (siblings.length > 1 ? ':nth-of-type(' + (siblings.indexOf(node) + 1) + ')' : '')); node = node.parentElement;
      }
      return node === document.body ? 'body > ' + parts.join(' > ') : '';
    };
    const entries = () => performance.getEntriesByType('resource').slice(-100).map(e => {
      let path = ''; try { path = new URL(e.name, location.href).pathname; } catch {}
      const status = Number.isInteger(e.responseStatus) && e.responseStatus >= 100 && e.responseStatus <= 599 ? e.responseStatus : null;
      return { url: url(e.name), path: safe(path, 1024), kind: safe(e.initiatorType, 40), status, durationMs: Math.round(Math.max(0, Number(e.duration) || 0)), transferBytes: Math.max(0, Number(e.transferSize) || 0) };
    });
    let result;
    if (a.op === 'snapshot') {
      invalidate(); state.stale = false; state.expiry = Date.now() + 60000; state.href = location.href;
      state.token = typeof crypto.randomUUID === 'function' ? crypto.randomUUID() : [...crypto.getRandomValues(new Uint32Array(4))].join('-');
      const elements = [], selector = 'a,button,input,textarea,select,h1,h2,h3,h4,h5,h6,[role],[tabindex]';
      const all = document.querySelectorAll(selector); let scanned = 0;
      for (const el of all) {
        if (++scanned > 2500 || elements.length >= 200) break;
        if (!visible(el) || el.matches('input[type=hidden]') || el.closest('[contenteditable]:not([contenteditable="false"])')) continue;
        const path = selectorOf(el); if (!path || path.length > 2048) continue;
        const ref = 'e' + (elements.length + 1); state.refs.set(ref, el);
        const tag = el.localName; const role = el.getAttribute('role') || ({ button: 'button', a: 'link', input: 'input', textarea: 'textbox', select: 'combobox' }[tag]) || tag;
        elements.push({ ref, selector: path, tag, role: safe(role, 60), name: nameOf(el), disabled: Boolean(el.disabled), inputType: tag === 'input' ? el.type : undefined });
      }
      state.observer = new MutationObserver(invalidate);
      state.observer.observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true });
      result = { token: state.token, text: textOf(document.body || document.documentElement, 12000), elements, truncated: scanned > 2500 || elements.length >= 200, scope: 'Visible light DOM only; not the platform accessibility tree. No form values.' };
    } else if (a.op === 'audit') {
      const findings = [], add = (code, message, el) => { if (findings.length < 100) findings.push({ code, message, selector: el ? selectorOf(el) : '' }); };
      if (!document.documentElement.lang.trim()) add('missing-language', 'Document language is not declared.');
      if (!document.title.trim()) add('missing-title', 'Document title is empty.');
      if (document.documentElement.scrollWidth > innerWidth + 2) add('horizontal-overflow', 'Content exceeds the visible viewport width.');
      const nodes = document.querySelectorAll('input,textarea,select,button,a,img,[role=button],[id]'), ids = new Set(); let scanned = 0;
      for (const el of nodes) {
        if (++scanned > 2000) break;
        if (el.id && ids.has(el.id)) add('duplicate-id', 'Duplicate element ID can break labels and selectors.', el); else if (el.id) ids.add(el.id);
        if (!visible(el) || el.matches('input[type=hidden]')) continue;
        if (el.localName === 'img') { if (!el.hasAttribute('alt')) add('image-alt', 'Image has no alt attribute; confirm whether it is decorative.', el); }
        else if (el.matches('button,a,input,select,textarea,[role=button]') && !nameOf(el) && !el.matches('input[type=submit],input[type=reset],input[type=button]')) add('missing-name', 'Interactive element has no detected accessible name.', el);
      }
      result = { findings, inspected: Math.min(scanned, 2000), truncated: scanned > 2000 || findings.length >= 100, scope: 'Heuristics only, not WCAG certification. Frames, shadow DOM, contrast and keyboard behavior need separate review.' };
    } else if (a.op === 'resources') {
      result = { resources: entries(), scope: 'Browser Resource Timing, at most 100 entries. No headers, bodies or query strings. Missing/zero HTTP status is unknown, never success.' };
    } else if (a.op === 'observe_start') {
      stop(); state.errors = [];
      const push = (kind, message, filename = '') => {
        state.errors.push({ kind, message: safe(message, 500), source: filename ? url(filename) : '', elapsedMs: Math.round(performance.now()) });
        if (state.errors.length > 80) state.errors.shift();
      };
      state.error = event => { if (event instanceof ErrorEvent) push('error', event.message, event.filename); else push('resource-error', 'A resource failed to load.'); };
      state.rejection = event => push('unhandled-rejection', typeof event.reason === 'string' ? event.reason : event.reason instanceof Error ? event.reason.message : 'Unhandled rejection (details omitted).');
      addEventListener('error', state.error, true); addEventListener('unhandledrejection', state.rejection);
      state.listening = true; state.timer = setTimeout(stop, 600000);
      result = { observing: true, expiresInSeconds: 600, scope: 'Errors after this action only. No console wrappers; navigation clears observation.' };
    } else if (a.op === 'observe_read') result = { observing: state.listening, errors: state.errors.slice(-80) };
    else if (a.op === 'observe_stop') { stop(); result = { observing: false }; }
    else if (a.op === 'clear') { stop(); invalidate(); state.errors = []; result = { cleared: true }; }
    else if (a.op.startsWith('expect_')) {
      const expected = a.value || ''; let passed = false, detail = 'Expected condition was not observed.';
      if (a.op === 'expect_text') passed = Boolean(expected) && textOf(document.body || document.documentElement, 24000).includes(expected);
      else if (a.op === 'expect_title') passed = Boolean(expected) && document.title.includes(expected);
      else if (a.op === 'expect_url') passed = Boolean(expected) && url(location.href).includes(expected);
      else if (a.op === 'expect_element') { const matches = document.querySelectorAll(a.selector || ''); passed = matches.length === 1 && visible(matches[0]); }
      else if (a.op === 'expect_resource') {
        if (!Number.isInteger(a.status) || a.status < 100 || a.status > 599 || !expected) throw new Error('A resource check needs a path and known HTTP status.');
        const matches = entries().filter(e => e.path === expected || e.url === expected), last = matches[matches.length - 1];
        passed = Boolean(last && last.status === a.status); detail = last?.status === null ? 'HTTP status is unavailable in this webview or cross-origin response.' : 'Last matching resource did not have the expected status, or was not observed.';
      }
      result = { passed, detail: passed ? 'Expected condition observed.' : detail };
    } else {
      if (a.op === 'scroll') {
        const positions = { up: -innerHeight * 0.8, down: innerHeight * 0.8, top: -document.documentElement.scrollHeight, bottom: document.documentElement.scrollHeight };
        if (!Object.hasOwn(positions, a.value)) throw new Error('Scroll must be up, down, top or bottom.');
        invalidate(); scrollBy({ top: positions[a.value], behavior: 'instant' }); result = { dispatched: true, input: 'DOM scroll' };
      } else {
        if (state.stale || state.href !== location.href || state.observer?.takeRecords().length || Date.now() > state.expiry || !a.token || a.token !== state.token) { invalidate(); throw new Error('Snapshot is stale. Take a new snapshot before acting.'); }
        const el = state.refs.get(a.ref);
        if (!el?.isConnected || !visible(el) || el.disabled || el.getAttribute('aria-disabled') === 'true') throw new Error('Element is unavailable. Take a new snapshot.');
        if (el.closest('[contenteditable]:not([contenteditable="false"])')) throw new Error('Editable documents are not supported.');
        if (el instanceof HTMLInputElement && (['password', 'file', 'hidden'].includes(el.type) || /(?:password|one-time-code|cc-|webauthn)/i.test(el.autocomplete))) throw new Error('Credential and file fields are not available to browser review tools.');
        const anchor = el.closest('a[href]'), form = el.form;
        if (a.op === 'click' && anchor && (!/^https?:$/.test(new URL(anchor.href).protocol) || new URL(anchor.href).origin !== origin || anchor.hasAttribute('download') || anchor.target === '_blank')) throw new Error('External navigation, downloads and new-window links are not allowed by this review.');
        if (a.op === 'click' && form && new URL(form.action || location.href, location.href).origin !== origin) throw new Error('Cross-origin form submission is not allowed by this review.');
        invalidate();
        if (a.op === 'click') el.click();
        else if (a.op === 'focus') el.focus();
        else if (a.op === 'fill') {
          if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) || el.readOnly || el instanceof HTMLInputElement && !['text','email','search','tel','url','number'].includes(el.type)) throw new Error('Select an editable non-secret text field.');
          const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
          Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, a.value || ''); el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true }));
        } else if (a.op === 'select') {
          if (!(el instanceof HTMLSelectElement) || ![...el.options].some(o => o.value === a.value && !o.disabled)) throw new Error('Select option is unavailable.');
          Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(el, a.value); el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true }));
        } else if (a.op === 'check') {
          if (!(el instanceof HTMLInputElement) || el.type !== 'checkbox' || !['true','false'].includes(a.value)) throw new Error('Check requires a checkbox and true/false.');
          if (el.checked !== (a.value === 'true')) el.click();
        }
        result = { dispatched: true, input: 'DOM events, not trusted OS pointer/keyboard input. Verify the resulting state.' };
      }
    }
    return JSON.stringify({ ...meta(), ...result });
  } catch (error) { return JSON.stringify({ ...meta(), ok: false, error: safe(error instanceof Error ? error.message : 'Browser review failed.', 500) }); }
})()
