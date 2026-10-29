import type { TaskStage } from './types';

export type SidebarMenuChoice = 'pin' | 'stage' | 'unread';
export interface SidebarMenuState { pinned: boolean; stage: TaskStage; unread: boolean }

/** A menu decision must still describe the task when the user applies it. */
export function sidebarMenuChoiceIsCurrent(choice: SidebarMenuChoice, opened: SidebarMenuState, current: SidebarMenuState): boolean {
  if (choice === 'pin') return opened.pinned === current.pinned;
  if (choice === 'stage') return opened.stage === current.stage;
  return opened.unread === current.unread;
}
