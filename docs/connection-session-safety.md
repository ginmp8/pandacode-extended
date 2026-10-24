# Named connections and fresh sessions

Choose a named connection in a task's harness/model picker. When the connection
changes, PandaCode keeps the visible conversation and project, clears the old
harness resume ID and native context metadata, and sends the existing bounded
context recovery with the next ordinary message. It never resumes a native
session using a different connection's credentials. Selecting the same connection
does not reset the context.

Editing or deleting a named connection also invalidates the contexts of its
assigned tasks. Stop affected agents and interactive harness consoles first.
Changing a connection is serialized with task admission. A connection must belong
to the task's current harness; remote tasks manage connections on their host.

The transcript and recovery marker are kept in SQLite; named connection settings
remain in their existing JSON file and secrets stay in the secure store. Context
invalidation is a single database transaction and commits before a JSON or secret
change. If the later write fails, the old assignment remains with a fresh context;
this intentionally favors an extra recovery over reusing another account's session.
No distributed transaction between SQLite, JSON and the OS secure store is claimed.

Recovery has the existing limit of 200 recent entries and 64 KiB. Private
reasoning, native tool state and image contents are not transferred; reattach images
when needed. Changing connections does not undo prior filesystem changes.

## Native controls with named Codex connections

The ordinary conversation and controls sent to its already authenticated running
Codex process keep using the chosen account. Interactive harness consoles now
apply the named connection's executable, arguments, configuration directory and
environment instead of silently using the global setup.

Some older Codex controls still construct an independent client for the default
account. For a task with a named connection, those paths now return an explicit
unsupported message before starting that client: idle saved-goal inspection;
standalone account usage/settings/hooks/imports/conversation/plugin/logout
commands; Plan/Fast preference changes that query that client's model catalog;
native fork/adopt, native archive/rename of an existing conversation; and opening
a saved Codex conversation in the console when it requires that goal inspection.
These controls remain available for the default connection. Ordinary chat,
running-agent goals/jobs, and the app's message Edit/Fork flow remain available.
Edit/Fork starts without a native resume ID; choose the desired connection before
sending its next message. This release does not silently fall back to the default
account or claim support for those named-account native controls.
