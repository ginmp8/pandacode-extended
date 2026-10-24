# Edit and rewind a sent prompt

Open a task and choose **Edit message** (the pencil beside Copy and Fork) on a
saved user message. PandaCode creates a new task containing only the conversation
before that prompt, then places the original text in the composer. Edit the text
and press Send when ready. The original task and its later messages remain intact.

The new task starts a fresh provider session. It does not copy the old resume ID
or authentication state; check the new task's connection before sending. History
is restored through the existing context recovery mechanism.

## Attachments and files

Saved document, project file, folder and browser context is restored as removable
**Saved attachment snapshot** chips. Their recorded contents are reused; PandaCode
does not reread a changed original file while preparing Edit. Images are restored
when their capture is still available. If an image predates stored image IDs or
its capture was cleaned up, PandaCode tells you to attach it again.

Both tasks use the current checkout. **Edit does not roll back files, commits,
branches or previously executed commands.** Stop the source agent and close its
harness console before editing. Native admission enforces this even if the task
starts while the composer is preparing the draft.

Editing a bot conversation also creates an independent task. It receives the
bot's current instructions and memories on its new provider context, but does not
inherit the original channel or send replies to the original desktop conversation.
Pausing the bot also prevents work in that fork. Fork context is bounded to 32 KiB.

The ordinary **Fork from here** action remains inclusive: it copies the selected
message too. Only Edit leaves the selected prompt out.

Bot conversation forks are listed in that bot’s Settings → Activity → Conversations, under **Independent tasks**. Opening one keeps its separate history and does not change the bot’s desktop or channel conversation.
