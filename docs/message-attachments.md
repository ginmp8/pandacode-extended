# Attachments in saved messages

Saved user messages show one chip for each file, document, folder, image,
screenshot, issue or pull request. Click a chip to open it:

- Project files open in the existing Files tab and use the task's workspace checks.
- External files and folders are revealed in the system file manager. PandaCode
  does not run or open their contents. Only a path recorded in that saved user
  message is eligible; traversal, network paths, missing items and final symlinks
  are refused. Remote task attachments do not operate on this computer's files.
- Stored images and screenshots open in an enlarged preview. Thumbnail reads are
  limited to two at a time. IDs are bound to the selected saved message; only
  bounded PNG files from the existing capture cache are read.
- GitHub issue and pull request chips open their HTTPS github.com item URL in the
  browser. Other context remains readable text.

The expandable **Attached context** detail retains the original text payload for
inspection and compatibility with older histories. Unknown formats stay readable
there. A dropped browser document whose original filesystem path is unknown opens
its saved text instead of a similarly named project file.

Older images without a stored image ID and captures removed by the existing cache
cleanup show the saved description. They cannot be reconstructed from text.
No database migration or change to the agent send API is required.

Legacy attachment records are textual context, not cryptographic provenance.
Both the UI and native lookup ignore context markers inside fenced code or
document snapshots. An unclosed fence keeps its remaining text opaque, and an
image ID inside extra prose or a fenced tail does not activate a cached image.
