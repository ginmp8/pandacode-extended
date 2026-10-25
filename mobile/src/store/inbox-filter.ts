// View-only filtering. Never changes task status, trusted hosts, drafts or the outbox.
import { filterBots, filterRows, type BotRow, type Inbox, type Section } from './inbox.ts';

export interface InboxFilters { query: string; deviceId: string; project: string }
export const projectKey = (deviceId: string, projectId: string): string => JSON.stringify([deviceId, projectId]);

export function filterInbox(inbox: Inbox, bots: readonly BotRow[], { query, deviceId, project }: InboxFilters): { inbox: Inbox; bots: BotRow[] } {
  const matches = (device: string, projectId: string) => (!deviceId || device === deviceId) && (!project || projectKey(device, projectId) === project);
  return {
    inbox: Object.fromEntries((Object.keys(inbox) as Section[]).map(section => [section,
      filterRows(inbox[section], query).filter(row => matches(row.deviceId, row.task.projectId)),
    ])) as Inbox,
    bots: filterBots(bots, query).filter(row => matches(row.deviceId, row.bot.projectId)),
  };
}

/** Project identity includes its computer; duplicate names/IDs on another host stay separate. */
export function inboxProjects(inbox: Inbox, bots: readonly BotRow[], deviceId: string): { value: string; label: string }[] {
  const projects = new Map<string, string>();
  const add = (device: string, id: string, name: string, computer: string) => {
    if (!deviceId || deviceId === device) projects.set(projectKey(device, id), deviceId ? name : `${name} · ${computer}`);
  };
  for (const rows of Object.values(inbox)) for (const row of rows) add(row.deviceId, row.task.projectId, row.task.chat ? 'Chat' : row.task.projectName, row.computer);
  for (const row of bots) add(row.deviceId, row.bot.projectId, `${row.bot.name} (bot)`, row.computer);
  return [...projects].map(([value, label]) => ({ value, label })).sort((a, b) => a.label.localeCompare(b.label));
}
