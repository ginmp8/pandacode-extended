# Shared Channel Council: footer popover (PandaCode Extended)

This guide covers configuring **Council for one message in a local Shared Channel**. The Council controls sit in a floating panel opened from the scales icon in the channel message composer, rather than an inline form that pushes the message field down.

**Availability:** The hybrid and nested Council additions are part of the Extended-only [PR #152](https://github.com/ginmp8/pandacode-extended/pull/152) candidate for [issue #147](https://github.com/ginmp8/pandacode-extended/issues/147). They are not an upstream PandaCode feature or evidence of a released desktop build. The exact-SHA acceptance ledger is in [Council #147 acceptance and architecture](council-147-acceptance.md).

**Evidence scope:** The selector wording below follows the current checkout, which includes the legacy reviewer radios. Those follow-up edits are not in the ledger's pinned PR head `5edec40ce913dc950dd50f0799e3aa1d63dc17a2`; that SHA's CI run failed and predates the selector restoration. The guide describes the current working tree, not a passing build.

> **About the images:** The four images below are browser-rendered **component previews** from PR #145, not screenshots captured in a running PandaCode desktop installation. They predate issue #147's composition controls and do not show all seven modes, three strategies or criteria sources. Their sample bots, messages and settings are illustrative; they demonstrate layout, not real harness execution.

## Open Council for a message

1. In **Bots**, open an active local Shared Channel that has members (for example, `#launch`).
2. Compose a message. If the message should be assigned to particular channel bots, mention up to two of them by name using `@mentions`.
3. Click the **scales/Council icon** at the bottom of the composer. This opens the floating Council panel without expanding an inline settings form.
4. Turn on the **Council** switch and choose a score and round limit. With schema-v1 composition off, choose **Automatic native reviewers** or **Selected channel bots**. To compose issue #147 modes, enable **Use a schema v1 composition plan** and configure reviewer sources, criteria and execution strategy separately.
5. Close the panel by clicking its icon again, clicking outside the popover, or pressing **Escape**; then send the message. There is no separate Save button. The settings are submitted with this message.

The toolbar icon reports **Council: off** or **Council: on**, reflecting whether the switch is enabled. Merely opening the panel does not enable Council.

![The Shared Channel message composer with the Council settings hidden and the scales icon in its footer](screenshots/channel-council-closed.webp)

*Figure 1 — Compact composer when the per-message Council panel is closed (component preview).*

## Configure scoring and reviewers

| Setting | Initial value | Controls and requirements |
| --- | --- | --- |
| Council | Off | Explicitly enable for the message that you are about to send. |
| Target score | `9.0` | Presets: `7.0`, `8.0`, `8.5`, `9.0`, `9.5` and `10.0`. A custom numeric field is available; valid values are from 1 to 10. |
| Max rounds | `10` | Presets: `3`, `5`, `10` and `25`. A custom integer field accepts 1 to 100 rounds. |
| Reviewers | **Automatic native reviewers** | With composition off, choose automatic native reviewers or **Selected channel bots**. The selected-bot path accepts 2–8 distinct eligible channel members, excluding a bot assigned to execute this message. With schema-v1 composition on, reviewer sources are configured independently; the derived mode combines automatic reviewers with each selected bot's **Direct review** or **Child Council** branch. |

The current implementation renders these two legacy labels in the Shared Channel composer. The current selected-bot submission test asserts a composition-off payload and that an out-of-range target blocks submission until corrected; it does not test the 2–8 roster bound or executor exclusion. See `src/bot-shared-channels.ts:306–315` and `tests/bots-channels.spec.ts:250–284`. These are current-checkout references, not evidence from the pinned `5edec40…` Actions run; native roster/self-review checks remain authoritative.

The selected preset is highlighted and the adjacent numeric field reflects it. Entering a custom number updates the preset highlight without discarding the value.

![Floating Council settings panel showing the enable switch, target score, max rounds and reviewer modes](screenshots/channel-council-panel.webp)

*Figure 2 — Floating settings panel; the underlying channel composer remains separate (component preview).*

When using **Selected channel bots**, select 2–8 distinct channel members. A bot assigned to execute the message cannot also review it. If too few eligible reviewers are available, adjust the channel membership or choose **Automatic native reviewers**. Selection identifies configured bots rather than creating new bot profiles.

![Council panel with Bots mode selected and two channel members checked as reviewers](screenshots/channel-council-selected-reviewers.webp)

*Figure 3 — Explicit reviewer selection for an individual message (component preview).*

## Compose hybrid and nested review

Schema-v1 composition is an explicit, per-message opt-in. When it is off, the message keeps the existing Council payload; opening the settings does not turn on a new reviewer source. In composition mode, three independent choices combine into the mode label:

| Mode | Root automatic reviewers | Selected direct bots | Selected bots with a child Council |
| --- | :---: | :---: | :---: |
| **M1 Architect** | Yes | No | No |
| **M2 Bots** | No | Yes | No |
| **M3 Bots + Architect** | Yes | Yes | No |
| **M4 Bots with own Architect** | No | No | Yes |
| **M5 Bots with own Architect + Architect** | Yes | No | Yes |
| **M6 Mixed bots** | No | Yes | Yes |
| **M7 Mixed bots + Architect** | Yes | Yes | Yes |

The label is derived from the sources; it is not a separate engine or a shortcut around validation. Select a channel member once and assign that bot to one branch. A task-executing bot cannot review its own message. The root Council requires **2–8 total seats**; automatic reviewers can be Auto or a fixed count from 1–8. Each nested bot occupies one root seat, regardless of its child reviewers. A child Council has **2–8** distinct reviewers and can run at one level only: children cannot start grandchildren.

Reviewer mode, criteria and execution strategy are separate settings. Choose among:

- **S1 Parallel (default):** run the enabled sources together. Every required root seat must pass.
- **S2 Escalated / on-demand:** begin with the enabled source set. Escalate on a below-target score, critical/high finding, missing required evidence, same-criterion conflict or user-forced trigger. A triggered escalation is required for approval. A nested bot can be set to **Always** or **On trigger**; S2 does not silently add a source that is disabled in the chosen mode.
- **S3 Two-phase:** after the first review, fresh independent reviewers challenge its findings against the same frozen artifact. Both phases must pass. If independent challengers are unavailable, the result remains blocked or inconclusive.

Choose **Architect-defined**, **User-defined** or **Hybrid** criteria independently of M1–M7 and S1–S3. User criteria are frozen task anchors. Hybrid criteria keep those anchors and may add nonduplicative Architect lenses. Each required criterion needs its required evidence and score; a per-criterion target cannot lower a stricter gate. A high average cannot compensate for a below-target seat or missing criterion. A child result is one composite parent seat, scored from the minimum eligible fresh child review, with its leaf reports available for drill-down.

Each child is a separate, task-owned Architect/Council session using frozen reviewer, criteria and policy inputs. It must return a typed result correlated to the parent cycle, seat, child cycle and artifact revision before the parent seat can settle. The parent does not pass while a child is reviewing, correcting or recovering. Reviewers are read-only by default; a user grant for native file inspection remains scoped to the exact relative path selected for this message. Composition does not reuse a bot's open conversation, change its profile or widen its permissions.

The Shared Channel plan fixes the hierarchy at `maxDepth: 2` and `maxRootSeats: 8`, with bounded concurrent leaves and review attempts, a 24-hour timeout and a cap of 1,000,000 estimated-cost micro-units. The plan preview is an estimate; native validation and receipts decide whether work may start and whether it passed. Missing or unknown usage is not treated as zero-cost evidence. See the [PR-head engineering schema-v1 notes](https://github.com/ginmp8/pandacode-extended/blob/5edec40ce913dc950dd50f0799e3aa1d63dc17a2/docs/architect-workers.md#schema-v1-composition) and [AC evidence ledger](council-147-acceptance.md) for implementation scope and current checks.

## Optional permissions and follow-up

These are independent, opt-in choices. Neither is enabled simply by opening the panel.

- **Post-acceptance GitHub issue:** Check **After Council approval, create one GitHub issue with accepted findings**, then enter the exact authorized `owner/repository`. The app uses the existing signed-in GitHub CLI and attempts a post-approval write. A failed or uncertain external write is reported separately and does not alter the Council verdict.
- **Scoped native file inspection (selected-reviewer flow):** Check **Allow one exact native file-inspection action** and specify a relative file path such as `src/example.ts`. This authorizes the narrowly scoped `pandacode_open_file` action for that path only. It does **not** authorize shell commands, checkout changes or unrestricted tool access.

The composer validates the score range, integer round limit, selected reviewer count, executor/reviewer separation and applicable path/repository inputs before dispatching an enabled Council request. Rejected settings show an error instead of silently relaxing the constraint. For the underlying reviewer authority and security boundary, see [native reviewer security (S1)](reviewer-security-s1.md) and [selected reviewers (S2)](council-selected-bots-s2.md).

## What happens after sending

Council settings, including a schema-v1 composition, apply to **the submitted message**, not to the Shared Channel, bot settings or later messages. Deliveries in the channel timeline can show an independent Council status, current round, minimum score and report/acceptance actions when available. Other channel messages and their deliveries do not inherit the Council configuration.

If the channel composer is re-rendered **while the unsent message remains on the same channel**, its Council inputs are restored and the panel stays closed. This is an in-memory draft behavior, **not** a promise of persistence across app restarts or a way to change a previously dispatched Council cycle.

The panel uses a non-modal popover. Keyboard focus moves to its switch when opened; pressing **Escape** dismisses it and returns focus to the footer icon. It is repositioned to fit within the viewport as the window resizes.

![A narrow-window Shared Channel preview with the Council popover open and reviewer selection visible](screenshots/channel-council-narrow.webp)

*Figure 4 — Narrow-window layout, with the Council panel constrained to the available viewport (component preview).*

## Troubleshooting

| Situation | What to check |
| --- | --- |
| The Council icon is absent | The channel must be active and have members. Archived or empty channels do not render this composer. |
| Council was not included with the sent message | Confirm the panel's Council switch is on before sending; opening the panel alone is insufficient. |
| Selected reviewers cannot be submitted | Choose 2–8 eligible members, excluding any bot assigned to execute the same message. |
| Target or round limit is rejected | Set the target between 1 and 10 and rounds to an integer between 1 and 100. |
| File inspection is rejected | Use a nonempty relative path with no parent-traversal segment; absolute paths and drive-qualified paths are not accepted. |
| GitHub issue creation fails | Verify the exact `owner/repository` and the existing GitHub CLI authentication and permissions. Approval and the external write have separate outcomes. |

For implementation details and test coverage, see the [engineering reference](shared-channel-council-engineering.md) and [Council #147 acceptance and architecture](council-147-acceptance.md). For Shared Channel collaboration and routing, see [Extended bot collaboration](extended-bot-collaboration.md).
