/** Browser preview for bots: in-memory sample data, no network, no channels. Native builds use src-tauri/src/bots. */
import type { Activity, Bot, BotDetail, BotInput, Channel, FailoverAsk, Memory, Pairing, Rule, Schedule, Thread } from './bots';
import type { Project, Session } from './types';
import type { SharedChannelDetail, SharedChannelMessage, SharedChannelSummary } from './bot-shared-channels';

export interface PreviewBotsHost {
  invoke<T>(command: string, args: Record<string, any>): Promise<T>;
  emit(name: string, event: unknown): void;
  addProject(name: string): Project;
  task(id: string): Session;
  started(id: string): boolean;
  isTeam(id: string): boolean;
  project(id: string): Project | undefined;
}
interface Store { bot: Bot; memories: Memory[]; schedules: Schedule[]; channels: Channel[]; pairings: Pairing[]; threads: Thread[]; rules: Rule[]; activity: Activity[] }
const bots = new Map<string, Store>();
const failoverAsks = new Map<string, FailoverAsk>();
const sharedChannels = new Map<string, SharedChannelDetail>();
let activityId = 1;
const MINUTE = 60_000;
const MODES = ['ask', 'auto', 'review', 'plan', 'full'];
const taskBots = new Map<string, string>();
const taskBriefVersions = new Map<string, number>();
export const taskBotId = (id: string): string | undefined => taskBots.get(id);
export function forgetBotTask(id: string): void { taskBots.delete(id); taskBriefVersions.delete(id); }
export function retryBotTask(id: string): void { taskBriefVersions.delete(id); }

function store(id: string): Store { const found = bots.get(id); if (!found) throw new Error('This bot no longer exists.'); return found; }
function log(entry: Store, kind: string, summary: string): void { entry.activity.unshift({ id: activityId++, kind, summary, detail: {}, at: Date.now() }); }
function validate(input: BotInput): BotInput {
  const name = String(input.name ?? '').trim().slice(0, 60);
  if (!name) throw new Error('Give the bot a name.');
  if (!MODES.includes(input.mode)) throw new Error('Choose a permission mode for the bot.');
  if (!input.provider) throw new Error('Choose a harness and model for the bot.');
  const soul = String(input.soul ?? '').trim();
  if (new TextEncoder().encode(soul).length > 64 * 1024) throw new Error('Personality must be under 64 KB.');
  return { ...input, name, soul, role: String(input.role ?? '').trim(), avatar: /^(panda:[0-9]|data:image\/png;base64,[A-Za-z0-9+/]+=*)$/.test(String(input.avatar ?? '')) && String(input.avatar ?? '').length <= 300_000 ? String(input.avatar) : [...String(input.avatar ?? '')].slice(0, 4).join(''), heartbeatMinutes: Number(input.heartbeatMinutes) || 0 };
}
function nextRun(input: Pick<Schedule, 'kind' | 'intervalMinutes' | 'timeOfDay'>, runAt: number | null): number | null {
  const now = Date.now();
  if (input.kind === 'interval') return input.intervalMinutes && input.intervalMinutes >= 5 && input.intervalMinutes <= 10080 ? now + input.intervalMinutes * MINUTE : null;
  if (input.kind === 'once') return runAt && runAt > now - MINUTE ? runAt : null;
  const match = /^(\d{2}):(\d{2})$/.exec(input.timeOfDay ?? '');
  if (!match || Number(match[1]) > 23 || Number(match[2]) > 59) return null;
  const next = new Date(); next.setHours(Number(match[1]), Number(match[2]), 0, 0);
  while (next.getTime() <= now || input.kind === 'weekdays' && [0, 6].includes(next.getDay())) next.setDate(next.getDate() + 1);
  return next.getTime();
}
function detail(entry: Store): BotDetail {
  return structuredClone({ bot: entry.bot, memories: entry.memories, schedules: entry.schedules, channels: entry.channels, pairings: entry.pairings, threads: entry.threads, rules: entry.rules, serverPort: 47321 });
}
function channelOf(id: string): [Store, Channel] {
  for (const entry of bots.values()) { const channel = entry.channels.find(item => item.id === id); if (channel) return [entry, channel]; }
  throw new Error('This channel no longer exists.');
}

/** A sample sender asks to pair shortly after a chat channel is added, so the flow can be tried. */
function simulatePairing(host: PreviewBotsHost, entry: Store, channel: Channel): void {
  if (channel.kind === 'web') return;
  window.setTimeout(() => {
    if (!entry.channels.includes(channel)) return;
    const pairing: Pairing = { channelId: channel.id, senderId: 'preview-sender', senderName: 'Sample sender', status: 'pending', code: '4821', createdAt: Date.now() };
    entry.pairings.unshift(pairing);
    host.emit('bot-pairing', { botId: entry.bot.id, channelId: channel.id, senderName: pairing.senderName, code: pairing.code });
    host.emit('bots-changed', null);
  }, 400);
}

/** Mirrors the native format checks; the preview never contacts a service. */
function previewIdentity(input: any): { name: string; link: string | null; invite: string | null; warnings: string[] } {
  const secret = (name: string) => String(input?.secrets?.[name] ?? '').trim();
  switch (input?.kind) {
    case 'telegram':
      if (!/^\d+:.{30,}$/.test(secret('token'))) throw new Error("That doesn't look like a Telegram bot token. It looks like 123456789:AAE… — copy the whole line BotFather sent.");
      return { name: '@preview_pandacode_bot', link: 'https://t.me/preview_pandacode_bot', invite: null, warnings: [] };
    case 'slack':
      if (secret('token').startsWith('xapp-') && secret('app_token').startsWith('xoxb-')) throw new Error('The two Slack tokens are swapped: the bot token starts with xoxb-, the app-level token with xapp-.');
      if (!secret('token').startsWith('xoxb-')) throw new Error('The bot token starts with xoxb-. Find it under OAuth & Permissions → Bot User OAuth Token (install the app first).');
      if (!secret('app_token').startsWith('xapp-')) throw new Error('The app-level token starts with xapp-. Create it under Basic Information → App-Level Tokens with the connections:write scope.');
      return { name: '@scout in Preview Workspace', link: 'https://preview.slack.com/', invite: null, warnings: [] };
    case 'discord':
      if (secret('token').split('.').length !== 3 || secret('token').length < 50) throw new Error("That doesn't look like a Discord bot token. In the Developer Portal open Bot → Reset Token and copy the whole value. (The Client Secret and Public Key are different values.)");
      return { name: 'Scout', link: null, invite: 'https://discord.com/oauth2/authorize?client_id=1234567890&scope=bot&permissions=274877975552', warnings: ['Message Content intent is off, so the bot cannot read messages. Turn it on at https://discord.com/developers/applications/1234567890/bot under Privileged Gateway Intents.'] };
    case 'whatsapp':
      if (!/^\d+$/.test(String(input?.config?.phoneNumberId ?? ''))) throw new Error('Enter the phone number ID (digits only) from WhatsApp → API Setup. It is not the phone number itself.');
      return { name: 'Preview Business (+1 555 0100)', link: 'https://wa.me/15550100', invite: null, warnings: [] };
    case 'web': return { name: 'Web chat on this computer', link: null, invite: null, warnings: [] };
    default: throw new Error('Choose web, Telegram, Slack, Discord or WhatsApp.');
  }
}

const briefed = new Map<string, number>();
async function desktopThread(host: PreviewBotsHost, entry: Store): Promise<Thread> {
  let thread = entry.threads.find(item => item.channelId === 'desktop');
  if (!thread) {
    const session = await host.invoke<Session>('create_session', { projectId: entry.bot.projectId, provider: entry.bot.provider, model: entry.bot.model, mode: entry.bot.mode });
    await host.invoke('update_session', { sessionId: session.id, title: `${entry.bot.name} · You` });
    thread = { channelId: 'desktop', externalId: 'desktop', sessionId: session.id, updatedAt: Date.now() };
    entry.threads.push(thread);
  }
  return thread;
}

/** Mirrors the native one-line preview of a bot conversation's latest message. */
async function lastMessage(host: PreviewBotsHost, sessionId: string): Promise<Bot['lastMessage']> {
  const page = await host.invoke<{ messages: { role: string; content: string; createdAt?: number; created_at?: number }[] }>('get_messages', { sessionId }).catch(() => null);
  const latest = [...(page?.messages ?? [])].reverse().find(message => (message.role === 'user' || message.role === 'assistant') && message.content.trim());
  if (!latest) return null;
  const line = latest.content.split(/\n\nBot brief:/)[0].replace(/[*_`#>]/g, '').split(/\s+/).filter(Boolean).join(' ');
  return { role: latest.role as 'user' | 'assistant', text: [...line].length > 160 ? [...line].slice(0, 160).join('') + '…' : line, at: latest.createdAt ?? latest.created_at ?? Date.now() };
}

const params = typeof location === 'undefined' ? new URLSearchParams() : new URLSearchParams(location.search);
let sampleSeeding: Promise<void> | null = null;
let sampleChannelSeeded = false;
/** Sample threads that look busy (their work is simulated by events, not a preview run), so a message waits in the queue as it would natively. */
const sampleBusy = new Set<string>();
/** Sample threads that show as working without a preview run behind them. */
export const previewBotBusy = (sessionId: string): boolean => sampleBusy.has(sessionId);
/** `?bots=sample` starts with four bots, one in each sidebar state: waiting for approval, working,
 *  at a usage limit, and paused. `?botsDelay=ms` holds the first list, to show the loading rows. */
async function seedSamples(host: PreviewBotsHost): Promise<void> {
  if (params.get('bots') !== 'sample') return;
  if (sampleSeeding) return sampleSeeding;
  sampleSeeding = (async () => {
  const base = { instructions: 'Sample bot.', goals: '', provider: 'claude', model: 'sonnet', mode: 'ask', heartbeatMinutes: 0 };
  const make = (name: string, role: string, avatar: string, profile: Partial<BotInput> = {}) => invokeBots<Bot>('bot_create', { input: { ...base, name, role, avatar, ...profile } }, host);
  const [waiting, working, limited, paused] = await Promise.all([make('Scout', 'Research', 'panda:5'), make('Writer', 'Drafts posts', 'panda:4'), make('Ledger', 'Finance', 'panda:7', { color: '#14b8a6', reasoningEffort: 'high', backupModels: [{ provider: 'codex', model: 'gpt-5' }], fallbackMode: 'auto', notify: false, whileWorking: 'steer', showPlan: true }), make('Dozy', 'Night shift', 'panda:8')]);
  if (!sampleChannelSeeded) {
    sampleChannelSeeded = true;
    const at = Date.now();
    sharedChannels.set('sample-launch', { id: 'sample-launch', name: 'launch', memberIds: [waiting.id, working.id, limited.id, paused.id], leadBotId: waiting.id, archived: false, updatedAt: at, activeBotIds: [], messages: [] });
  }
  await invokeBots('bot_set_paused', { botId: paused.id, paused: true }, host);
  const [ask, busy, capped] = await Promise.all([waiting, working, limited].map(bot => invokeBots<string>('bot_desktop_thread', { botId: bot.id }, host)));
  sampleBusy.add(ask); sampleBusy.add(busy);
  // Once the window has the new threads, they report live work like real runs do.
  window.setTimeout(() => {
    host.emit('agent-event', { sessionId: ask, kind: 'approval', requestId: 'sample-approval', text: 'Run `curl https://example.com`?' });
    host.emit('agent-event', { sessionId: busy, kind: 'status', text: 'Working…' });
    host.emit('agent-event', { sessionId: capped, kind: 'limited', text: 'Usage limit reached', data: { resetsAt: Date.now() + 3 * 3_600_000 } });
  }, 600);
  })();
  await sampleSeeding;
}

/** Stops a sample channel's work (only `botIds`' assignments, when given), as Stop, archive and member removal do natively. */
function stopChannelWork(channel: SharedChannelDetail, botIds?: string[]): void {
  const mine = (item: SharedChannelDetail['messages'][number]) => !botIds || botIds.includes(item.authorBotId ?? '');
  for (const marker of channel.messages.filter(item => item.kind === 'assigned' && (item.status === 'In progress' || item.status === 'Queued') && mine(item))) marker.status = 'Stopped';
  const latest = [...channel.messages].reverse().find(item => item.kind === 'user' && item.status === 'In progress');
  channel.activeBotIds = channel.activeBotIds.filter(id => botIds && !botIds.includes(id));
  if (latest && !channel.activeBotIds.length) latest.status = 'Stopped';
}

function sharedChannel(id: string): SharedChannelDetail {
  const channel = sharedChannels.get(id);
  if (!channel) throw new Error('Shared channel not found.');
  return channel;
}

function channelSummary(channel: SharedChannelDetail): SharedChannelSummary {
  const { id, name, memberIds, leadBotId, archived, updatedAt } = channel;
  return { id, name, memberIds: [...memberIds], leadBotId, archived, updatedAt };
}

function validateChannelMembers(value: unknown): string[] {
  if (!Array.isArray(value) || value.some(id => typeof id !== 'string' || !bots.has(id))) throw new Error('Choose valid channel members.');
  return [...new Set(value as string[])];
}

function validateChannelName(value: unknown): string {
  const name = String(value ?? '').trim().replace(/^#+/, '');
  if (!name || [...name].length > 48) throw new Error('Enter a channel name under 49 characters.');
  return name;
}

export async function invokeBots<T>(command: string, args: Record<string, any>, host: PreviewBotsHost): Promise<T> {
  const changed = () => host.emit('bots-changed', null);
  let result: unknown = null;
  switch (command) {
    case 'bots_list': await seedSamples(host); if (params.has('botsDelay')) await new Promise(done => window.setTimeout(done, Number(params.get('botsDelay')) || 0)); result = await Promise.all([...bots.values()].map(async entry => {
      const desktopSession = entry.threads.find(thread => thread.channelId === 'desktop')?.sessionId ?? null;
      return { ...structuredClone(entry.bot), desktopSession, lastMessage: desktopSession ? await lastMessage(host, desktopSession) : null };
    })); break;
    case 'bot_shared_channels': {
      await seedSamples(host);
      result = [...sharedChannels.values()].sort((a, b) => b.updatedAt - a.updatedAt).map(channelSummary);
      break;
    }
    case 'bot_shared_channel_detail': {
      const channel = sharedChannel(String(args.channelId));
      const limit = Math.max(1, Math.min(1000, Number(args.limit) || 100));
      result = { ...channel, memberIds: [...channel.memberIds], activeBotIds: [...channel.activeBotIds],
        messages: channel.messages.slice(-limit), hasMore: channel.messages.length > limit };
      break;
    }
    case 'bot_shared_channel_create': {
      const name = validateChannelName(args.name), memberIds = validateChannelMembers(args.memberIds);
      const id = crypto.randomUUID(), updatedAt = Date.now();
      const channel: SharedChannelDetail = { id, name, memberIds, leadBotId: memberIds[0] ?? null,
        archived: false, updatedAt, activeBotIds: [], messages: [] };
      sharedChannels.set(id, channel);
      result = channelSummary(channel);
      host.emit('bot-shared-channel-changed', { channelId: id });
      break;
    }
    case 'bot_shared_channel_update': {
      const channel = sharedChannel(String(args.channelId));
      if (channel.archived) throw new Error('This channel is deleted.');
      const name = validateChannelName(args.name), memberIds = validateChannelMembers(args.memberIds);
      const removed = channel.memberIds.filter(id => !memberIds.includes(id));
      if (removed.length) stopChannelWork(channel, removed);
      channel.name = name;
      channel.memberIds = memberIds;
      channel.leadBotId = memberIds.includes(args.leadBotId) ? args.leadBotId : memberIds[0] ?? null;
      channel.updatedAt = Date.now();
      result = channelSummary(channel);
      host.emit('bot-shared-channel-changed', { channelId: channel.id });
      break;
    }
    case 'bot_shared_channel_archive': {
      const channel = sharedChannel(String(args.channelId));
      channel.archived = Boolean(args.archived);
      if (channel.archived) stopChannelWork(channel);
      channel.updatedAt = Date.now();
      host.emit('bot-shared-channel-changed', { channelId: channel.id });
      break;
    }
    case 'bot_desktop_thread': { const entry = store(args.botId); const known = entry.threads.length; result = (await desktopThread(host, entry)).sessionId; if (entry.threads.length !== known) { host.emit('sessions-changed', null); changed(); } break; }
    case 'bot_project_ids': result = [...bots.values()].map(entry => entry.bot.projectId); break;
    case 'bot_detail': {
      const entry = store(args.botId), tasks = await host.invoke<Session[]>('list_sessions', {});
      const owned = new Set(entry.threads.map(thread => thread.sessionId));
      result = { ...detail(entry), independentTasks: tasks.filter(task => task.projectId === entry.bot.projectId && !owned.has(task.id) && !task.draft).sort((a, b) => b.updatedAt - a.updatedAt).slice(0, 500).map(({ id, title, updatedAt }) => ({ id, title, updatedAt })) };
      break;
    }
    case 'bot_create': {
      const input = validate(args.input);
      const id = crypto.randomUUID(), now = Date.now();
      const project = host.addProject(input.name);
      const bot: Bot = { ...input, soul: input.soul ?? '', mcp: [], id, projectId: project.id, paused: false, memoryVersion: 0, notifyChannelId: null, notifyThread: null, nextHeartbeatAt: input.heartbeatMinutes ? now + input.heartbeatMinutes * MINUTE : null, createdAt: now, updatedAt: now };
      const entry: Store = { bot, memories: [], schedules: [], channels: [], pairings: [], threads: [], rules: [], activity: [] };
      log(entry, 'created', `Created ${bot.name}`);
      bots.set(id, entry); result = structuredClone(bot); changed(); break;
    }
    case 'bot_update': {
      const entry = store(args.botId); const input = validate(args.input);
      const changedBrief = ['name', 'role', 'instructions', 'goals', 'soul'].some(key => entry.bot[key as keyof Bot] !== input[key as keyof BotInput]);
      entry.bot = { ...entry.bot, ...input, memoryVersion: entry.bot.memoryVersion + Number(changedBrief), updatedAt: Date.now() };
      log(entry, 'updated', 'Settings changed'); result = structuredClone(entry.bot); changed(); break;
    }
    case 'bot_set_capabilities': {
      const entry = store(args.botId), values = args.mcp;
      if (!Array.isArray(values) || values.length > 50 || values.some(value => typeof value !== 'string' || !value.trim() || new TextEncoder().encode(value.trim()).length > 100 || /[\x00-\x1f\x7f]/.test(value))) throw new Error('Use up to 50 bounded MCP server names.');
      const mcp = [...new Set<string>(values.map(value => value.trim()))];
      if (JSON.stringify(mcp) !== JSON.stringify(entry.bot.mcp)) entry.bot = { ...entry.bot, mcp, memoryVersion: entry.bot.memoryVersion + 1, updatedAt: Date.now() };
      result = structuredClone(entry.bot); changed(); break;
    }
    case 'bot_assign_task': {
      const task = host.task(args.sessionId);
      if (task.remoteDevice || host.project(task.projectId)?.path.includes('/PandaCode Chats/') || [...bots.values()].some(entry => entry.bot.projectId === task.projectId)) throw new Error('Choose a bot for a local project task.');
      if (host.isTeam(task.id) || host.started(task.id) || task.resumeId || task.archived || task.taskStage === 'completed' || ['running', 'waiting'].includes(task.status)) throw new Error("Choose a bot before this ordinary task's first message.");
      const bot = args.botId ? store(args.botId).bot : null;
      if (bot?.paused) throw new Error('Resume this bot before assigning it to a task.');
      if (bot) { await host.invoke('configure_session', { sessionId: task.id, provider: bot.provider, model: bot.model }); taskBots.set(task.id, bot.id); }
      else taskBots.delete(task.id);
      taskBriefVersions.delete(task.id); result = structuredClone(bot); changed(); host.emit('sessions-changed', null); break;
    }
    case 'bot_task_links': result = Object.fromEntries(taskBots); break;
    case 'bot_set_paused': { const entry = store(args.botId); entry.bot = { ...entry.bot, paused: Boolean(args.paused) }; log(entry, 'updated', args.paused ? 'Paused' : 'Resumed'); result = structuredClone(entry.bot); changed(); break; }
    case 'bot_reset': { const entry = store(args.botId); const threads = [...new Set(entry.threads.map(thread => thread.sessionId))];
      for (const sessionId of threads) await host.invoke('delete_session', { sessionId });
      entry.memories = []; entry.schedules = []; entry.threads = []; entry.activity = []; log(entry, 'reset', 'Reset: conversations, memories and schedules cleared');
      result = structuredClone(entry.bot); changed(); host.emit('sessions-changed', null); break; }
    case 'bot_delete': { const entry = store(args.botId); const threads = [...new Set(entry.threads.map(thread => thread.sessionId))];
      for (const sessionId of threads) await host.invoke('delete_session', { sessionId });
      bots.delete(args.botId); for (const [id, botId] of taskBots) if (botId === args.botId) forgetBotTask(id);
      changed(); host.emit('sessions-changed', null); break; }
    case 'bot_remember': {
      const entry = store(args.botId); const content = String(args.content ?? '').trim().slice(0, 2000);
      if (!content) throw new Error('A memory needs some text.');
      const existing = entry.memories.find(memory => memory.content.toLowerCase() === content.toLowerCase());
      const memory = existing ?? { id: crypto.randomUUID(), content, source: 'user' as const, createdAt: Date.now() };
      if (!existing) { entry.memories.push(memory); entry.bot.memoryVersion++; log(entry, 'memory', `Remembered: ${content.slice(0, 120)}`); }
      result = memory; changed(); break;
    }
    case 'bot_forget': { const entry = store(args.botId); const before = entry.memories.length; entry.memories = entry.memories.filter(memory => memory.id !== args.memoryId); if (before === entry.memories.length) throw new Error('That memory was already removed.'); log(entry, 'memory', 'Forgot a memory'); changed(); break; }
    case 'bot_activity': { const entry = store(args.botId); result = entry.activity.filter(item => args.before == null || item.id < args.before).slice(0, 100); break; }
    case 'bot_message': {
      const entry = store(args.botId); const text = String(args.text ?? '').trim();
      if (!text) throw new Error('Send some text.');
      if (entry.bot.paused) throw new Error('This bot is paused.');
      const thread = await desktopThread(host, entry);
      thread.updatedAt = Date.now();
      log(entry, 'inbound', `You (desktop): ${text.slice(0, 160)}`);
      // Like the native gateway, the first message of a thread carries the bot's brief; the transcript folds it.
      const brief = briefed.get(thread.sessionId) === entry.bot.memoryVersion ? '' : profileBrief(entry.bot, false);
      briefed.set(thread.sessionId, entry.bot.memoryVersion);
      const prompt = text + brief, images = (args.imageIds ?? []).map((id: string) => ({ kind: 'image', id, name: 'Attached image' }));
      const task = await host.invoke<Session>('get_task', { sessionId: thread.sessionId });
      if (task.status === 'running' || task.status === 'waiting' || sampleBusy.has(thread.sessionId)) {
        if (args.forceQueue === true) {
          await host.invoke('queue_add', { sessionId: thread.sessionId, text: prompt, attachments: images });
          host.emit('queue-changed', { sessionId: thread.sessionId });
        } else if (entry.bot.whileWorking === 'steer' && !images.length && await host.invoke('steer_task', { sessionId: thread.sessionId, text: prompt }).then(() => true, () => false)) { /* Steered. */ }
        else {
          await host.invoke('queue_add', { sessionId: thread.sessionId, text: prompt, attachments: images });
          host.emit('queue-changed', { sessionId: thread.sessionId });
        }
      } else await host.invoke('send_message', { sessionId: thread.sessionId, text: prompt, imageIds: args.imageIds ?? [], moodboardIds: args.moodboardIds ?? [] });
      result = thread.sessionId; changed(); break;
    }
    case 'bot_schedule_create': {
      const entry = store(args.botId); const input = args.input ?? {};
      const prompt = String(input.prompt ?? '').trim();
      if (!prompt) throw new Error('A schedule needs a prompt: what should the bot do?');
      const next = nextRun(input, input.runAt ?? null);
      if (next == null) throw new Error('Check the schedule\'s time: intervals are 5 minutes to a week, times are HH:MM, one-off runs are within a year.');
      const schedule: Schedule = { id: crypto.randomUUID(), botId: entry.bot.id, title: String(input.title ?? '').trim() || prompt.slice(0, 60), prompt, kind: input.kind, intervalMinutes: input.intervalMinutes ?? null, timeOfDay: input.timeOfDay ?? null, nextRunAt: next, lastRunAt: null, enabled: true, createdBy: 'user', createdAt: Date.now() };
      entry.schedules.push(schedule); log(entry, 'schedule', `Scheduled: ${schedule.title}`); result = schedule; changed(); break;
    }
    case 'bot_schedule_set_enabled': {
      for (const entry of bots.values()) { const schedule = entry.schedules.find(item => item.id === args.scheduleId); if (schedule) { schedule.enabled = Boolean(args.enabled); schedule.nextRunAt = schedule.enabled ? nextRun(schedule, schedule.nextRunAt) : schedule.nextRunAt; result = schedule; } }
      if (!result) throw new Error('This schedule no longer exists.'); changed(); break;
    }
    case 'bot_schedule_delete': { const entry = store(args.botId); entry.schedules = entry.schedules.filter(item => item.id !== args.scheduleId); changed(); break; }
    case 'bot_run_schedule_now': {
      const entry = [...bots.values()].find(item => item.schedules.some(schedule => schedule.id === args.scheduleId));
      const schedule = entry?.schedules.find(item => item.id === args.scheduleId);
      if (!entry || !schedule) throw new Error('This schedule no longer exists.');
      if (entry.bot.paused) throw new Error('This bot is paused.');
      schedule.lastRunAt = Date.now(); log(entry, 'scheduled', `Scheduled: ${schedule.title}`); changed(); break;
    }
    case 'bot_channel_test': result = previewIdentity(args.input); break;
    case 'bot_channel_configure': {
      const [, channel] = channelOf(args.channelId);
      for (const [key, value] of Object.entries(args.config ?? {})) {
        if (key === 'remote' && channel.kind === 'web') channel.config = { ...channel.config, remote: Boolean(value) };
        else if (key === 'publicUrl' && ['web', 'whatsapp'].includes(channel.kind)) {
          const url = String(value ?? '').trim().replace(/\/+$/, '');
          if (url && !/^https:\/\/[^/?#@\s]+$/.test(url)) throw new Error('Use the https:// address your tunnel or Tailscale shows, without a path or query.');
          channel.config = { ...channel.config, publicUrl: url };
        } else throw new Error(`${key} cannot be changed for this channel.`);
      }
      result = channel; changed(); break;
    }
    case 'bot_channel_add': {
      const entry = store(args.botId); const input = args.input ?? {};
      const identity = previewIdentity(input);
      if (!['web', 'telegram', 'slack', 'discord', 'whatsapp'].includes(input.kind)) throw new Error('Choose web, Telegram, Slack, Discord or WhatsApp.');
      const required: Record<string, string[]> = { telegram: ['token'], discord: ['token'], slack: ['token', 'app_token'], whatsapp: ['access_token', 'signing_secret'], web: [] };
      if (input.kind === 'whatsapp' && !/^\d+$/.test(String(input.config?.phoneNumberId ?? ''))) throw new Error("Enter the WhatsApp phone number ID from Meta's API setup page.");
      for (const name of required[input.kind]) if (!String(input.secrets?.[name] ?? '').trim()) throw new Error(`Enter the ${name.replace('_', ' ')} for this channel.`);
      const config = { ...(input.config ?? {}), identity, ...(input.kind === 'whatsapp' ? { verifyToken: 'pandacode-preview0123456789' } : {}) };
      const channel: Channel = { id: crypto.randomUUID(), botId: entry.bot.id, kind: input.kind, label: String(input.label ?? '').trim(), config, enabled: true, status: input.kind === 'web' ? 'Ready on this computer' : 'Preview · not connected', secrets: required[input.kind] };
      entry.channels.push(channel); log(entry, 'channel', `Added a ${input.kind} channel`); result = channel; changed();
      simulatePairing(host, entry, channel); break;
    }
    case 'bot_channel_set_enabled': { const [, channel] = channelOf(args.channelId); channel.enabled = Boolean(args.enabled); channel.status = channel.enabled ? 'Preview · not connected' : 'Off'; result = channel; changed(); break; }
    case 'bot_channel_remove': { const [entry] = channelOf(args.channelId); entry.channels = entry.channels.filter(item => item.id !== args.channelId); entry.pairings = entry.pairings.filter(item => item.channelId !== args.channelId); log(entry, 'channel', 'Removed a channel'); changed(); break; }
    case 'bot_pairing_decide': {
      const [entry] = channelOf(args.channelId);
      if (!['approved', 'blocked', 'removed'].includes(args.decision)) throw new Error('Approve, block or remove the sender.');
      entry.pairings = args.decision === 'removed' ? entry.pairings.filter(item => !(item.channelId === args.channelId && item.senderId === args.senderId)) : entry.pairings.map(item => item.channelId === args.channelId && item.senderId === args.senderId ? { ...item, status: args.decision } : item);
      log(entry, 'pairing', `Sender ${args.decision}`); changed(); break;
    }
    case 'bot_web_new_link': { const [entry, channel] = channelOf(args.channelId); if (channel.kind !== 'web') throw new Error('Only web channels have a chat link.'); log(entry, 'channel', 'Created a new web chat link; old links stopped working'); changed(); result = `http://127.0.0.1:47321/chat/${channel.id}#preview-token-${crypto.randomUUID().slice(0, 8)}`; break; }
    case 'bot_web_link': { const [, channel] = channelOf(args.channelId); if (channel.kind !== 'web') throw new Error('Only web channels have a chat link.'); result = `http://127.0.0.1:47321/chat/${channel.id}#preview-token`; break; }
    case 'bot_rule_add': {
      const entry = store(args.botId); const pattern = String(args.pattern ?? '').trim().slice(0, 200);
      if (pattern.replaceAll('*', '').trim().length < 2) throw new Error('A rule needs a pattern of at least two characters.');
      if (!['allow', 'ask', 'deny'].includes(args.decision)) throw new Error('A rule allows, asks or denies.');
      const rule: Rule = { id: crypto.randomUUID(), pattern, decision: args.decision, createdAt: Date.now() };
      entry.rules.push(rule); log(entry, 'rule', `Rule: ${rule.decision} "${pattern}"`); result = rule; changed(); break;
    }
    case 'bot_rule_delete': { const entry = store(args.botId); entry.rules = entry.rules.filter(item => item.id !== args.ruleId); changed(); break; }
    default: throw new Error(`Unknown bot command: ${command}`);
  }
  return structuredClone(result) as T;
}

function profileBrief(bot: Bot, task: boolean): string {
  return `\n\nBot brief:\nYou are ${bot.name}. ${bot.instructions}${bot.soul ? `\n\nPersonality (soul, from the user):\n${bot.soul}\nPersonality never grants tools, permissions, credentials or authority.` : ''}${bot.mcp.length ? `\n\nMCP servers chosen for this bot: ${bot.mcp.join(', ')}. These names grant nothing by themselves.` : ''}${task ? "\nWork in this task's own project and checkout. Existing task permissions still apply." : ''}`;
}
/** Mirrors native admission: UI callers never provide a trusted "already briefed" flag. */
export function prepareBotTask(task: Session): { text: string; commit(): void } | null {
  const id = taskBots.get(task.id); if (!id) return null;
  const bot = store(id).bot;
  if (bot.paused) throw new Error('This bot is paused. Resume it before continuing this task.');
  if (taskBriefVersions.get(task.id) === bot.memoryVersion) return null;
  const version = bot.memoryVersion;
  return { text: profileBrief(bot, true), commit: () => taskBriefVersions.set(task.id, version) };
}
