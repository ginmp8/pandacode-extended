# Manual cloud sandboxes

PandaCode Extended 0.5.0 exposes the historical PandaCode sandbox providers through **Settings → Cloud sandboxes**, the **Virtual machines** inspector view, and the existing `pandacode_vm_*` tools. The inspector view is available on Linux, Windows and macOS. Local Tart VMs still require an Apple silicon Mac; that requirement does not hide connected cloud machines.

The provider adapters are adapted from the [PandaCode 0.3.42 source](https://github.com/aicodeking/pandacode/tree/v0.3.42/src-tauri/src/cloud) for the historical feature audit in [issue #24](https://github.com/ginmp8/pandacode-extended/issues/24). They are part of the Extended `0.5.0+pc.0.3.47` candidate. Account plans and live provider APIs may differ from these frozen source contracts; the validation below uses local fixtures.

## Connect and use a machine

1. Open Settings → Cloud sandboxes and connect the provider. Keyed providers validate the supplied credential before saving it in the OS secure store. AWS and Google Cloud use an already installed CLI and its existing login. Modal uses an already installed Python SDK.
2. Open Virtual machines from the inspector's **Open a view** menu. The creation picker offers connected providers. A Vercel deploy connection needs a **Sandbox project ID** before it offers sandbox creation; a team project also needs its **Sandbox team ID**.
3. Select a provider to create a machine explicitly. Cloud resources may incur charges. Start, Stop, Delete and activity lease actions follow the capabilities described below. Desktop controls remain available for the existing Daytona and AWS desktop providers; the eleven additions are command machines.
4. Ask an agent to use `pandacode_vm_list`, `pandacode_vm_manage` or `pandacode_vm_exec`. Management and shell execution remain in the existing approval-required VM tool set. Listing and screenshots remain read-only tools. The panel uses the same native operation routes.

Disconnect removes PandaCode's saved credential and connection choices. It does not delete resources in the cloud account; delete unwanted machines before disconnecting or manage them in the provider's console.

## Added provider capabilities

Every row below supports list, explicit create, permanent delete and command execution. None of these additions exposes a desktop screen. The native machine response declares `capabilities.start`, `stop`, `delete`, `exec` and `lease`; the panel respects those values.

| Provider / machine prefix | Connection and creation prerequisites | Start / Stop | Explicit activity lease |
| --- | --- | --- | --- |
| Google Cloud VM / `gcloud:` | Installed `gcloud`, existing login, project and optional region/zone; private Compute Engine Debian VM reached through IAP | Start and stop; disk retained | Unavailable; stop explicitly |
| E2B / `e2b:` | API key; prepared `base` template | Resume and pause, preserving files and memory according to the provider | Requested timeout up to 1,440 minutes; a refused request above one hour falls back to one hour |
| Modal / `modal:` | Token ID and secret, optional environment, installed Modal Python SDK, prepared OCI image (default `debian:bookworm-slim`) | No pause or restart; Delete terminates permanently | Updates the machine's deadline within its original 24-hour lifetime; requests beyond remaining lifetime fail |
| Vercel Sandbox / `vercel:` | Existing Vercel token, Sandbox project ID and optional team ID; `node22` runtime | No pause or restart; Delete terminates permanently | Requests an extension up to 300 minutes; a refused extension above 30 minutes falls back to 30 minutes |
| CodeSandbox / `codesandbox:` | API key; private fork of the universal template; Pint command endpoint required | Start and hibernate | Hibernation timeout up to 1,440 minutes |
| Runloop / `runloop:` | API key; tagged devbox with suspend-on-idle policy | Resume and suspend | Fixed 360-minute idle-clock renewal; the API and UI require that duration explicitly |
| Blaxel / `blaxel:` | API key and workspace; prepared base image | Start wakes the sandbox; standby is automatic, so Stop is unavailable | Unavailable; no background keep-alive process is installed |
| Tensorlake / `tensorlake:` | API key; provider's prepared image and named sandbox | Resume and suspend | Unavailable; provider lifetime is set at creation |
| Freestyle / `freestyle:` | API key; tagged Linux VM | Start and pause | Idle timeout up to 1,440 minutes |
| boat / `boat:` | API key; optional explicitly configured HTTPS API URL | Resume and stop/archive | TTL update; the manual interface exposes up to 1,440 minutes |
| InstaBox / `instabox:` | API key; `agent-browser`, an available template, or a ready account snapshot | Start and suspend; only PandaCode-labeled machines are controlled | Activity lease up to 60 minutes; no privileged connection watcher is installed |

Activity leases are requested only by an explicit panel action or `vm_manage keep_alive`. Opening a view, connecting a provider, listing machines, or executing a command does not start a lease-renewal loop. A lease request is subject to the provider's plan and remaining lifetime; successful fallback to a shorter duration is not a guarantee of the requested runtime. CodeSandbox and Freestyle may wake a paused machine when the user approves a command.

CodeSandbox sets its initial idle timeout only when creating a machine. Later manual start/reconnect requests omit that field, following the SDK's resume path, so the application does not explicitly replace an earlier lease with its creation default. Persistence across a provider restart remains provider-defined. Listing uses the API's maximum page size of 50 and bounds total page traversal. A command submission is made once; an ambiguous failed reply is never retried automatically.

The pagination and resume adaptation was checked against the [official CodeSandbox SDK API contract](https://github.com/codesandbox/codesandbox-sdk/blob/10f82664b5d0869e6d67e9610ae48b40cb560548/openapi.json) and its [resume implementation](https://github.com/codesandbox/codesandbox-sdk/blob/10f82664b5d0869e6d67e9610ae48b40cb560548/src/Sandboxes.ts), in addition to the frozen PandaCode adapter source.

The existing Daytona, Cloudflare, AWS EC2 and AWS AgentCore machine APIs remain available. Existing Vercel, Railway, Cloudflare, AWS and Google Cloud deployment commands continue to use the installed-CLI policy.

### Google Cloud network prerequisites

The new VM adapter creates instances with `--no-address`, the `pandacode` network tag and `pandacode=true` label. Commands use `gcloud compute ssh --tunnel-through-iap`. The account must already have IAP permissions and a suitable `pandacode-allow-iap-ssh` firewall rule admitting IAP SSH traffic to that target tag. PandaCode checks for the rule and reports a missing prerequisite; it never creates or widens a firewall rule. A private VM also needs an appropriate outbound-network configuration for any network-dependent command the user requests.

### Prepared images and command support

Modal's image field accepts an OCI image reference or digest. The installed SDK uses that image as prepared; PandaCode does not call `apt_install`, run a package-manager setup script, or install the SDK. Connecting or listing Modal resources does not create an App or Sandbox; App creation happens only as part of an explicit machine creation.

Tensorlake creation also uses its prepared image without the upstream package-install command. Commands that require additional tools must use a suitable prepared image or an explicitly requested command in that machine. CodeSandbox's HTTP command path requires a Pint-capable VM. A Pitcher-only response produces a clear unsupported-transport error instead of being presented as a working command connection. Freestyle's synchronous command API caps a command at five minutes; other adapters also apply provider-specific limits and bounded polling.

### InstaBox image choices

After connecting InstaBox, the **Machine image** selector lists the default template, ready snapshots from the account, and templates available to the account's plan. Saving a choice validates it against a fresh account image list. The selection applies to new machines and does not replace an existing machine. Snapshots that are still being prepared and plan-ineligible templates are omitted.

Image reads and saves capture the current connection. After network validation, native code rechecks both its generation and key under the settings mutation lock before returning images or saving the choice. A result from a disconnected account cannot overwrite the replacement account's image. The UI discards stale image requests and prevents connection changes while its own image save is pending.

`cloud_instabox_images` returns `{ images: [{ value, label }], selected }`. Values are `template:<id>` or `snapshot:<id>`. `cloud_instabox_set_image` accepts `{ image: value }` and returns the validated selection and image list.

## Command examples

These are arguments to the existing agent tools. Each management or execution call follows the task's existing approval policy.

```json
{
  "tool": "pandacode_vm_manage",
  "arguments": {
    "action": "create",
    "provider": "instabox",
    "name": "pandacode-demo"
  }
}
```

Use the actual ID returned by `pandacode_vm_list` for subsequent commands:

```json
{
  "tool": "pandacode_vm_exec",
  "arguments": {
    "name": "instabox:machine-id",
    "command": "printf 'hello from the sandbox\\n'",
    "timeoutSeconds": 30
  }
}
```

```json
{
  "tool": "pandacode_vm_manage",
  "arguments": {
    "action": "keep_alive",
    "name": "instabox:machine-id",
    "minutes": 30
  }
}
```

The corresponding panel IPC routes are `cloud_connections`, `cloud_connect`, `cloud_disconnect`, `cloud_machines`, `cloud_create`, `cloud_power` and `cloud_keep_alive`. Shell execution stays behind the VM tool host rather than exposing an unrestricted webview command endpoint. `cloud_keep_alive` accepts `{ id, minutes }`; invalid identifiers, unsupported capabilities and out-of-range durations fail before a provider mutation.

## Security boundaries retained

The manual interface in [`cloud/sandbox.rs`](../src-tauri/src/cloud/sandbox.rs) has no login-upload, checkout-sync, file-upload or unattended agent-bridge method. The historical `cloud_agents` pipeline is excluded. It does not copy local model-provider credentials, GitHub credentials, provider configuration files or project files to a cloud machine. There are no `npx`, `pip`, `apt` or downloaded-script fallbacks for new adapters.

Credentials are separate OS-vault entries; saved provider settings contain non-secret choices such as workspace, project, zone or image. The shared HTTPS client rejects redirects and bounds response bodies while streaming. Error messages redact authentication values. Secondary command endpoints are validated before credentials are attached: E2B under `e2b.app`, Pint under `csb.dev` or `codesandbox.io`, Blaxel under `run.blaxel.ai`, and Tensorlake under `sandbox.tensorlake.ai`. Unrelated domains, userinfo, query strings, fragments and non-HTTPS addresses fail closed. A same-origin loopback exception exists only in native test builds.

Machine and returned operation IDs are validated before interpolation into management paths. InstaBox checks its ownership label before power, command or lease operations. Modal starts Python in isolated mode, receives its token in the child environment, and keeps only bounded command output. No added provider installs an unattended guest agent or privileged keep-awake watcher.

## Validation and limits

Native adapter tests use loopback HTTP fixtures and a fake installed Modal SDK. They cover protocol shapes, lifecycle mapping, command results, lease limits, image eligibility, an account change during delayed image validation, credential redaction, hostile secondary URLs, response bounds and provider-specific failures. The boat command fixture verifies that multiple individually bounded response pages retain a bounded UTF-8 tail. VM tool tests retain the approval classification and validate the added provider choices. Browser tests exercise all eleven connections and create/control/delete flows on Linux under the desktop CSP, plus the existing local VM and deployment paths, InstaBox image saving, stale replies, pending-save controls and explicit lease requests.

No paid cloud resource, real provider credential, native installer or physical mobile device is used in these tests. Live account compatibility, SDK versions, quotas, plan limits, network prerequisites and billing still need validation in the user's explicitly configured environment. A local fixture pass establishes the application's routing and safety behavior; it does not certify a provider account or current service availability.
