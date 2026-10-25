This is an Expo/React Native mobile application. Prioritize mobile-first patterns, performance, and cross-platform compatibility.

## Expo has changed — do not trust your training data

Expo ships breaking changes every SDK release. APIs you remember are likely renamed, moved, or removed. Before writing any code that touches an Expo, EAS, or React Native API:

1. Read the major version of the `expo` package in `package.json`.
2. Fetch the matching versioned docs: `https://docs.expo.dev/versions/v<major>.0.0/`
3. For anything else, fetch https://docs.expo.dev/llms.txt — an index of all Expo docs with corrections to common LLM misconceptions. Follow its links to the specific page you need; never answer from memory.

## Commands

The checked-in npm lockfile is authoritative. Preserve the reviewed upstream
versions and Extended's `ignore-scripts` policy. Use the installed local package
scripts; do not bootstrap tools from the network.

```bash
npm ci --ignore-scripts
npm run start
npm test
npm run typecheck
EXPO_NO_TELEMETRY=1 npm run lint
```

Run lint and typecheck before declaring any task done.

## Navigation & Routing

- Use **Expo Router** for all navigation. Routes live in `src/app/` — every file there is a screen, `_layout.tsx` files define navigators. Keep non-route code (components, hooks, utils) outside `src/app/`.
- Import `Link`, `router`, and `useLocalSearchParams` from `expo-router`.
- Docs: https://docs.expo.dev/router/introduction.md

## Native builds and publication

Use `npm run ios` or `npm run android` with a reviewed local SDK/toolchain for
native validation. The Extended package intentionally has no EAS owner/project,
cloud submission profile or hosted release destination. Automatic app updates
are disabled. Do not register a cloud project, build in a publisher's account,
submit to a store or deploy updates as part of development/CI. A maintainer must
explicitly configure and authorize publication separately.

## Rules

- If `ios/` and `android/` directories do not exist, they are generated (Continuous Native Generation). Never create or edit them by hand — configure native behavior in `app.json` and config plugins.
- Expo Go only includes its bundled native modules. Native code needs a local development build through `npm run ios` or `npm run android`.
- Prefer recommended Expo modules over third-party libraries, and check your available skills before adding dependencies. Docs: https://docs.expo.dev/versions/latest/index.md
