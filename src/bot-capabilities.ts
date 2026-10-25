import './skills-library.css';
import { escape } from './dom';
import type { Bot, BotsOptions } from './bots';
import type { CapabilityInventory } from './capabilities-panel';
import { mountSkillScopes } from './skills-scope';

/** Bot settings reuse Extended's reviewed content pins. MCP choices contain names
 * only; the harness still owns server configuration, credentials and approvals. */
export function mountBotCapabilities(host: HTMLElement, bot: Bot, options: BotsOptions): () => void {
  host.innerHTML = '<div data-bot-skills></div><section data-bot-mcp></section>';
  const skills = mountSkillScopes(host.querySelector<HTMLElement>('[data-bot-skills]')!, options, { target: 'bot', id: bot.id });
  const mcpHost = host.querySelector<HTMLElement>('[data-bot-mcp]')!;
  let disposed = false, busy = false, loading = true, error = '', note = '';
  let configured = new Set<string>();
  const chosen = new Set(bot.mcp);
  function draw(): void {
    if (disposed) return;
    const names = [...new Set([...configured, ...chosen])].sort();
    mcpHost.innerHTML = `<h3 class="bs-heading">MCP servers</h3><div class="bs-group"><p class="bots-empty-note">Choose names from this harness's local configuration. This selection does not install servers, copy credentials or change permissions.</p>
      <form aria-label="Bot MCP selection"><ul class="skills-catalog-list">${names.map(name => `<li><label class="skills-toggle"><input type="checkbox" value="${escape(name)}" aria-label="${escape(`Use MCP ${name}`)}" ${chosen.has(name) ? 'checked' : ''} ${busy ? 'disabled' : ''}><span><strong>${escape(name)}</strong><small>${configured.has(name) ? 'Configured for this harness' : 'Not found for this harness; retained until you remove it'}</small></span></label></li>`).join('')}</ul>
      ${!names.length ? `<p class="bots-empty-note">${loading ? 'Reading configured servers…' : 'No configured MCP servers found for this harness.'}</p>` : ''}
      <button type="submit" class="secondary-button" ${busy || loading ? 'disabled' : ''}>Save MCP selection</button><p class="skills-note" role="status">${escape(note)}</p><p class="skills-error" role="alert" ${error ? '' : 'hidden'}>${escape(error)}</p></form></div>`;
  }
  const change = (event: Event) => {
    const input = event.target;
    if (busy || !(input instanceof HTMLInputElement) || input.type !== 'checkbox') return;
    if (input.checked) chosen.add(input.value); else chosen.delete(input.value);
  };
  const submit = async (event: Event) => {
    event.preventDefault(); event.stopPropagation();
    if (busy || disposed || loading) return;
    busy = true; error = ''; note = ''; draw();
    try { await options.call('bot_set_capabilities', { botId: bot.id, mcp: [...chosen] }); note = 'MCP selection saved for future turns.'; }
    catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
    busy = false; draw();
  };
  mcpHost.addEventListener('change', change); mcpHost.addEventListener('submit', submit);
  draw();
  void options.call<CapabilityInventory>('capabilities_inventory', { taskId: null }).then(inventory => {
    if (disposed) return;
    configured = new Set(inventory.entries.filter(entry => entry.harness === bot.provider && entry.kind === 'mcp server').map(entry => entry.name));
  }).catch(cause => { if (!disposed) error = cause instanceof Error ? cause.message : String(cause); }).finally(() => { if (!disposed) { loading = false; draw(); } });
  return () => { disposed = true; skills(); mcpHost.removeEventListener('change', change); mcpHost.removeEventListener('submit', submit); };
}
