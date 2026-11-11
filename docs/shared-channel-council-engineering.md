# Shared Channel Council popover: implementation and verification

**Scope:** UI work in [PR #145](https://github.com/ginmp8/pandacode-extended/pull/145) on the Extended feature branch, not a redesign of the native Council executor. The [user guide](shared-channel-council.md) provides illustrated steps; the images there are **component previews**, not desktop-run validation evidence.

## Source ownership

| File | Responsibility |
| --- | --- |
| `src/bot-shared-channels.ts` | Renders the channel composer and the Council popover; binds the switch, segmented presets, Escape/focus handling and viewport positioning; restores unsent form values when the same channel re-renders. |
| `src/bot-shared-channels.css` | Sizes and styles the footer trigger, popover and responsive controls using the existing design system. |
| `src/main.ts` | Binds a newly rendered composer, delegates the `channel-council` trigger, validates the per-message Council payload and posts the channel message. |
| `tests/bot-collaboration.spec.ts` | Checks floating layout, keyboard and viewport behavior, input preservation, and independent delivery/status rendering using preview fixtures. |
| `tests/bots-channels.spec.ts` | Exercises the channel UI, FormData, score/round presets, selected-reviewer controls and refresh behavior in the browser preview. |

The direct-bot Council remains implemented separately in `src/council-panel.ts` and `src/council.css`. The channel popover reuses visual classes but **does not replace or alter the direct conversation's Council**.

## Rendering and state contract

1. `sharedChannelComposerHtml` renders a footer button (class `bc-council-trigger`) and a `section` using native `popover="auto"` and `role="dialog"`, outside the visible message field but **inside the same HTML form**.
2. `bindSharedChannelCouncil` synchronizes the trigger's `Council: on/off` accessible label, its active state and selected preset buttons with the underlying Council inputs. Clicking a preset changes its numeric input, rather than introducing a second value source.
3. `toggleSharedChannelCouncil` uses `showPopover`/`hidePopover`. The button reports `aria-expanded`; the switch receives initial focus. Escape closes the panel and returns focus to the trigger. Native light dismissal supports closing outside the panel.
4. `positionSharedChannelCouncilPanel` anchors to the footer icon and chooses space above or below, constraining width to 380 px, height to 560 px and horizontal viewport margins to at least 12 px. Repositioning runs while the panel is open on resize.
5. A channel refresh may replace the composer DOM. `restoreSharedChannelCouncilInputs` copies the prior unsent Council inputs **only if both forms belong to the same channel**; `bindSharedChannelCouncil` then reattaches handlers and starts with the popover closed. This is not durable setting persistence.

Keeping fields under `#shared-channel-composer` is intentional: the existing `FormData(form)` submission path continues reading all Council inputs even when their popover is hidden. The UI change is not a channel-wide preference and does not modify the native reviewer authority contract.

## Dispatch and validation contract

In the `shared-channel-composer` submit handler in `src/main.ts`:

- The Council object is omitted unless `councilEnabled` is checked; default target is 9 and maximum rounds is 10.
- Targets must be finite and within 1–10; round counts must be integers within 1–100. The target input exposes half-point increments and both fields retain manual entry.
- In selected mode, `reviewerBotIds` must contain 2–8 selected bots. A selected reviewer cannot be one of the current message's assigned executors.
- Optional file authorization is available only in selected-reviewer mode and requires a valid, nonempty relative path (maximum 240 characters, no absolute/drive paths, empty segments or `..` parent segments). The emitted grant is limited to `pandacode_open_file` for the exact requested path.
- Optional GitHub issue authorization requires a valid `owner/repository` and is represented by a per-message post-acceptance authorization ID. The actual external-write outcome remains separate from acceptance.
- When enabled, a `council` object is attached to the existing `bot_shared_channel_post` call. The native Council, bot queues, security guards, reviewers and persisted delivery/cycle outcomes are not reimplemented by this popover.

This describes the browser submission contract, **not** a claim that client validation replaces native authorization checks.

## Focused verification

From the feature branch checkout, with project dependencies available, the relevant source-level checks are:

```sh
npm ci
npm run build
npm test
npx playwright test tests/bot-collaboration.spec.ts tests/bots-channels.spec.ts
```

The Playwright suites cover the composer visibility, opening/closing, Escape focus restoration, selected preset values, custom numbers, refresh restoration, reviewer mode and status isolation. Browser previews and mocks are useful UI regression evidence, **not** proof that real reviewer providers, signed-in GitHub writes or Desktop-native file grants executed successfully.

There is no requirement for an interactive manual test before further code work; any eventual live Desktop/provider verification must be tracked independently of these preview checks. Changes to this UI should preserve the native Council submission shape and existing reviewer authority contracts.

## Update checklist for maintainers

- Preserve the trigger/dialog accessibility attributes and the existing native popover behavior.
- Keep all values in the same HTML form so message submission uses a single source of truth.
- Re-test same-channel refresh, Escape focus, narrow viewport placement and selected reviewer bounds.
- Maintain the preview caveat in the [illustrated guide](shared-channel-council.md) until screenshots from a real Desktop session replace those component previews.
- Preserve per-message opt-in semantics; do not silently change channel-wide settings or bot permissions.
