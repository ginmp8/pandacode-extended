/** Narrow existing-IPC adapters; no execution, approval or configuration commands. */
import { sameTeamSetup } from '../team-mode.ts';
import { buildReview, reviewProblems, type ReviewRequest } from './review-model.ts';
import type { TeamLibrary, TeamSetup, TeamSnapshot } from '../architect-types';
import type { MessagePage, Provider, Session } from '../types';

export type ReviewCommand = 'architect_profiles' | 'architect_save_profile' | 'get_task' | 'get_messages' | 'architect_get';
export type ReviewCall = <T>(command: ReviewCommand, args?: Record<string, unknown>) => Promise<T>;

export async function saveReviewProfile(call: ReviewCall, name: string, setup: TeamSetup): Promise<{ created: boolean }> {
  const trimmed = name.trim();
  if (!trimmed || [...trimmed].length > 80 || /[\u0000-\u001f\u007f\u202a-\u202e\u2066-\u2069]/.test(trimmed)) throw new Error('Profile names must be 1-80 characters without controls.');
  const captured = structuredClone(setup);
  const library = await call<TeamLibrary>('architect_profiles');
  const existing = library.profiles.find(p => p.name.toLowerCase() === trimmed.toLowerCase());
  if (existing) {
    // The original helper does not compare these newer flags; include them explicitly.
    if (!sameTeamSetup(existing.setup, captured) || existing.setup.config.auto !== captured.config.auto || existing.setup.config.rosterOnly !== captured.config.rosterOnly || existing.setup.config.name !== captured.config.name) {
      throw new Error('A different profile already exists with that name. Choose another name; nothing was overwritten.');
    }
    return { created: false };
  }
  await call<TeamLibrary>('architect_save_profile', { id: null, name: trimmed, setup: captured });
  return { created: true };
}

/** Reread authoritative state immediately before preparing text. The host must still
 * verify the selected task and empty composer after the await. This never sends text. */
export async function prepareReviewPrompt(call: ReviewCall, taskId: string, providers: Provider[], request: ReviewRequest): Promise<string> {
  const plan = buildReview(request);
  const [task, page, team] = await Promise.all([
    call<Session>('get_task', { sessionId: taskId }),
    call<MessagePage>('get_messages', { sessionId: taskId }),
    call<TeamSnapshot | null>('architect_get', { sessionId: taskId }),
  ]);
  if (task.id !== taskId) throw new Error('The native task identity changed. Select the task again.');
  const problems = reviewProblems({ task, team, providers, hasMessages: page.hasOlder || page.messages.length > 0 }, request);
  if (problems.length) throw new Error(problems.join(' '));
  return plan.prompt;
}
