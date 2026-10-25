export interface AccountDeletionContext<Socket> {
  getWebSockets(): Iterable<Socket>;
  storage: { deleteAll(): Promise<void> };
}

/** A successful response follows durable deletion, never only a socket close. */
export async function deleteAccountState<Socket>(
  ctx: AccountDeletionContext<Socket>,
  close: (socket: Socket, code: number, reason: string) => void,
): Promise<void> {
  for (const socket of ctx.getWebSockets()) close(socket, 4001, "signed out");
  await ctx.storage.deleteAll();
}
