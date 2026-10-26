import { expect, test, type Page } from '@playwright/test';
import { openInspectorTab } from './inspector-tab-helper';
import { readFileSync } from 'node:fs';

// The inspector's VM panel in the browser preview: Tart setup, creating and starting a
// VM, its live screen, and mouse, wheel and keyboard input mapped to screen pixels.
const errors = new WeakMap<Page, string[]>();
const calls = (page: Page, command: string): Promise<any[]> => page.evaluate(command => (window as any).__vmCalls.filter((entry: any) => entry.command === command).map((entry: any) => entry.args), command);
const panel = (page: Page) => page.getByRole('region', { name: 'Virtual machines', exact: true });
const control = (page: Page, name: string) => panel(page).getByRole('toolbar', { name: 'VM controls' }).getByRole('button', { name, exact: true });

const cspViolations = new WeakMap<Page, string[]>();
test.afterEach(async ({ page }) => { expect(errors.get(page) ?? []).toEqual([]); expect(cspViolations.get(page) ?? []).toEqual([]); });

async function open(page: Page, platform = 'MacIntel', previewHook = ''): Promise<void> {
  // Pick the desktop platform explicitly: local Tart on macOS, cloud machines everywhere.
  await page.addInitScript(platform => Object.defineProperty(Navigator.prototype, 'platform', { get: () => platform }), platform);
  const runtimeErrors: string[] = []; errors.set(page, runtimeErrors);
  page.on('pageerror', error => runtimeErrors.push(error.message));
  // Native vm_frame returns JPEG bytes. Keep the preview's VM lookup/running checks,
  // but use a recorded 1024×768 preview frame instead of encoding one asynchronously.
  // The panel still decodes and paints it under the desktop CSP before showing it.
  const frame = readFileSync('tests/fixtures/vm-screen.jpg').toString('base64');
  await page.route('**/src/preview-vm.ts*', async route => {
    const response = await route.fetch(); const source = await response.text();
    const marker = /async function frame\(\) \{/g;
    expect(source.match(marker)).toHaveLength(1);
    await route.fulfill({ response, body: source.replace(marker, `async function frame() {
      return Uint8Array.from(atob(${JSON.stringify(frame)}), byte => byte.charCodeAt(0)).buffer;
    `) });
  });
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch(); const source = await response.text();
    const marker = /async function invoke\(command, args\) \{/;
    expect(source).toMatch(marker);
    await route.fulfill({ response, body: source.replace(marker, `async function invoke(command, args) {
      if (command.startsWith('vm_') || command.startsWith('cloud_')) (window.__vmCalls ??= []).push({ command, args: structuredClone(args) });
      ${previewHook}
      return invokePreview(command, args);
    }
    async function invokePreview(command, args) {`) });
  });
  // The desktop app's Content Security Policy, so the panel is tested under the same rules.
  const csp = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).app.security.csp.replace("connect-src ipc: http://ipc.localhost", "connect-src 'self' ws: ipc: http://ipc.localhost");
  await page.route(url => new URL(url).pathname === '/', async route => {
    const response = await route.fetch();
    await route.fulfill({ response, body: (await response.text()).replace('<head>', `<head><meta http-equiv="Content-Security-Policy" content="${csp}">`) });
  });
  const blocked: string[] = [];
  page.on('console', message => { if (/Content Security Policy/i.test(message.text())) blocked.push(message.text()); });
  cspViolations.set(page, blocked);
  await page.goto('/');
  await expect(page.locator('#prompt')).toBeVisible();
  if (!(await page.locator('#inspector').isVisible())) await page.locator('#inspector-toggle').click();
  await openInspectorTab(page, 'tab-vm');
  await page.locator('.inspector-tabs [data-action="tab-vm"]').click();
  await expect(page.locator('#inspector-content')).toBeHidden();
  await expect(panel(page)).toBeVisible();
}

async function createAndStart(page: Page): Promise<void> {
  await panel(page).getByRole('button', { name: 'Set up VMs' }).click();
  await expect(panel(page).getByText('Create a machine', { exact: true })).toBeVisible();
  await panel(page).getByRole('button', { name: /^Ubuntu 24\.04/ }).click();
  // A new VM starts at once. Starting it never installs desktop packages.
  await expect(panel(page).getByRole('img', { name: 'VM screen' })).toBeVisible();
  expect((await calls(page, 'vm_create'))[0]).toEqual({ image: 'ubuntu', name: 'pandacode-ubuntu' });
  await expect(panel(page).getByRole('button', { name: 'Choose VM' })).toContainText('pandacode-ubuntu');
}

test('VM start does not install packages; Add a desktop remains an explicit action', async ({ page }) => {
  await open(page);
  await expect(panel(page).getByText('Run Linux, macOS and Windows machines')).toBeVisible();
  await createAndStart(page);
  await expect(control(page, 'Stop VM')).toBeVisible();
  expect(await calls(page, 'vm_desktop')).toEqual([]);
  await expect(panel(page).locator('.vm-status')).not.toContainText('Installing a desktop');
  await expect(control(page, 'Add a desktop')).toBeVisible();
  await control(page, 'Stop VM').click();
  await control(page, 'Start VM').click();
  await expect(panel(page).locator('.vm-status')).not.toContainText('Installing a desktop');
  expect(await calls(page, 'vm_desktop')).toEqual([]);
  await control(page, 'Add a desktop').click();
  await expect.poll(() => calls(page, 'vm_desktop')).toEqual([{ name: 'pandacode-ubuntu' }]);
  await expect(panel(page).getByRole('status')).toContainText('Desktop ready');
  await expect(control(page, 'Suspend VM')).toBeHidden();
  const size = await panel(page).getByRole('img', { name: 'VM screen' }).evaluate((canvas: HTMLCanvasElement) => [canvas.width, canvas.height]);
  expect(size).toEqual([1024, 768]);
  await control(page, 'Stop VM').click();
  await expect(panel(page).getByText('pandacode-ubuntu is stopped')).toBeVisible();
  await expect(panel(page).getByRole('img', { name: 'VM screen' })).toBeHidden();
});

test('clicks, drags, scrolls and typing reach the VM in screen pixels', async ({ page }) => {
  await open(page);
  await createAndStart(page);
  const screen = panel(page).getByRole('img', { name: 'VM screen' });
  const box = (await screen.boundingBox())!;
  // The middle of the shown image is the middle of the 1024×768 screen.
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  // WebKit reports whole CSS pixels; one of them spans several screen pixels in the small panel.
  const slack = Math.ceil(1024 / box.width) + 1;
  const near = (value: number, expected: number) => Math.abs(value - expected) <= slack;
  await expect.poll(async () => { const input = (await calls(page, 'vm_input')).at(-1)?.input; return input?.action === 'click' && near(input.x, 512) && near(input.y, 384); }).toBe(true);
  await page.mouse.move(box.x + box.width * 0.25, box.y + box.height * 0.25);
  await page.mouse.down(); await page.mouse.move(box.x + box.width * 0.75, box.y + box.height * 0.5, { steps: 5 }); await page.mouse.up();
  await expect.poll(async () => { const input = (await calls(page, 'vm_input')).at(-1)?.input; return input?.action === 'drag' && near(input.x, 256) && near(input.y, 192) && near(input.endX, 768) && near(input.endY, 384); }).toBe(true);
  await page.mouse.click(box.x + 10, box.y + 10, { button: 'right' });
  await expect.poll(async () => (await calls(page, 'vm_input')).at(-1)?.input.action).toBe('right_click');
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, 120);
  await expect.poll(async () => (await calls(page, 'vm_input')).at(-1)?.input).toMatchObject({ action: 'scroll', direction: 'down', amount: 3 });
  // Typed characters arrive together; special keys and shortcuts as key presses.
  await page.keyboard.type('ls -la');
  await page.keyboard.press('Enter');
  await page.keyboard.press('Control+c');
  await expect.poll(async () => (await calls(page, 'vm_input')).slice(-3).map(call => call.input)).toEqual([{ action: 'type', text: 'ls -la' }, { action: 'key', keys: 'Return' }, { action: 'key', keys: 'ctrl+c' }]);
});

test('shares the project read-only when asked, and a VM can be deleted', async ({ page }) => {
  await open(page);
  await panel(page).getByRole('button', { name: 'Set up VMs' }).click();
  await panel(page).getByRole('button', { name: /^Debian/ }).click();
  await expect(panel(page).getByRole('img', { name: 'VM screen' })).toBeVisible();
  await control(page, 'Stop VM').click();
  await expect(panel(page).getByText('pandacode-debian is stopped')).toBeVisible();
  await control(page, 'Share project read-only at start').click();
  await expect(control(page, 'Share project read-only at start')).toHaveAttribute('aria-pressed', 'true');
  await control(page, 'Start VM').click();
  await expect(panel(page).getByRole('img', { name: 'VM screen' })).toBeVisible();
  const start = (await calls(page, 'vm_power')).filter(call => call.action === 'start').at(-1);
  expect(start.share).toMatch(/^\//);
  await panel(page).getByRole('button', { name: 'Choose VM' }).click();
  await page.getByRole('menuitemradio', { name: 'Delete pandacode-debian' }).click();
  await expect(panel(page).getByText('Create a machine', { exact: true })).toBeVisible();
});

async function connect(page: Page, provider: string, fields: Record<string, string>, sandboxes = false): Promise<void> {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const settings = page.getByRole('region', { name: 'Settings', exact: true });
  await settings.getByRole('tab', { name: 'Cloud sandboxes', exact: true }).click();
  await settings.getByRole('button', { name: `Connect ${provider}`, exact: true }).click();
  const form = settings.getByRole('form', { name: `Connect ${provider}` });
  for (const [label, value] of Object.entries(fields)) await form.getByRole('textbox', { name: `${provider} ${label}` }).or(form.getByLabel(`${provider} ${label}`)).fill(value);
  await form.locator('[type="submit"]').click();
  await expect(settings.getByRole('button', { name: `Disconnect ${provider}`, exact: true })).toBeVisible();
  if (sandboxes) {
    await settings.getByRole('button', { name: 'Set up sandboxes', exact: true }).click();
    await expect(settings.getByText(/sandboxes ready/)).toBeVisible();
  }
  await settings.getByRole('button', { name: 'Back to the workspace', exact: true }).click();
  await expect(settings).toBeHidden();
}

test('connecting a cloud provider checks the key and offers its machines', async ({ page }) => {
  await open(page);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const settings = page.getByRole('region', { name: 'Settings', exact: true });
  await settings.getByRole('tab', { name: 'Cloud sandboxes', exact: true }).click();
  await settings.getByRole('button', { name: 'Connect Daytona', exact: true }).click();
  const form = settings.getByRole('form', { name: 'Connect Daytona' });
  await form.locator('[type="submit"]').click();
  await expect(form.getByRole('alert')).toHaveText('Enter the api key.');
  await form.getByLabel('Daytona API key').fill('bad');
  await form.locator('[type="submit"]').click();
  await expect(form.getByRole('alert')).toContainText('refused the credentials');
  await form.getByLabel('Daytona API key').fill('dtn_good');
  await form.getByLabel('Daytona Organization ID').fill('org-1');
  await form.locator('[type="submit"]').click();
  await expect(settings.getByRole('button', { name: 'Disconnect Daytona', exact: true })).toBeVisible();
  const connect = (await calls(page, 'cloud_connect')).at(-1);
  expect(connect).toEqual({ provider: 'daytona', key: 'dtn_good', options: { organization: 'org-1', apiUrl: '' } });
  await settings.getByRole('button', { name: 'Back to the workspace', exact: true }).click();
  await expect(settings).toBeHidden();
  // The panel now offers a Daytona desktop even before Tart is set up.
  await expect(panel(page).getByRole('button', { name: /^Daytona desktop/ })).toBeVisible();
});

test('a cloud desktop streams its screen and takes input; a commands-only sandbox explains itself', async ({ page }) => {
  await open(page);
  await connect(page, 'Daytona', { 'API key': 'dtn_good' });
  await connect(page, 'Cloudflare', { 'API token': 'cf_token', 'Account ID': '0123456789abcdef0123456789abcdef' }, true);
  await panel(page).getByRole('button', { name: /^Daytona desktop/ }).click();
  const screen = panel(page).getByRole('img', { name: 'VM screen' });
  await expect(screen).toBeVisible();
  expect((await calls(page, 'cloud_create'))[0]).toMatchObject({ provider: 'daytona' });
  const box = (await screen.boundingBox())!;
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  const slack = Math.ceil(1280 / box.width) + 1;
  await expect.poll(async () => { const input = (await calls(page, 'cloud_input')).at(-1)?.input; return input?.action === 'click' && Math.abs(input.x - 640) <= slack && Math.abs(input.y - 400) <= slack; }).toBe(true);
  await control(page, 'Stop VM').click();
  await expect.poll(async () => (await calls(page, 'cloud_power')).at(-1)).toMatchObject({ action: 'stop' });
  await expect(panel(page).getByText(/is stopped/)).toBeVisible();
  await control(page, 'New VM').click();
  await page.getByRole('menuitemradio', { name: 'Cloudflare container' }).click();
  await expect(panel(page).getByText(/has no screen/)).toBeVisible();
  await expect(control(page, 'Share project read-only at start')).toBeHidden();
});

const manualProviders = [
  { id: 'gcloud', name: 'Google Cloud', label: 'Google Cloud VM', fields: { 'Project ID': 'fixture-project', Region: 'us-central1', Zone: 'us-central1-a' } },
  { id: 'e2b', name: 'E2B', label: 'E2B', fields: { 'API key': 'fixture-key' } },
  { id: 'modal', name: 'Modal', label: 'Modal', fields: { 'Token secret': 'fixture-secret', 'Token ID': 'ak-fixture', 'Prepared image': 'debian:bookworm-slim' }, noStop: true },
  { id: 'vercel', name: 'Vercel', label: 'Vercel Sandbox', fields: { 'Access token': 'fixture-token', 'Sandbox project ID': 'prj_fixture', 'Sandbox team ID': 'team_fixture' }, noStop: true },
  { id: 'codesandbox', name: 'CodeSandbox', label: 'CodeSandbox', fields: { 'API key': 'fixture-key' } },
  { id: 'runloop', name: 'Runloop', label: 'Runloop', fields: { 'API key': 'fixture-key' } },
  { id: 'blaxel', name: 'Blaxel', label: 'Blaxel', fields: { 'API key': 'fixture-key', Workspace: 'fixture-workspace' }, noStop: true },
  { id: 'tensorlake', name: 'Tensorlake', label: 'Tensorlake', fields: { 'API key': 'fixture-key' } },
  { id: 'freestyle', name: 'Freestyle', label: 'Freestyle', fields: { 'API key': 'fixture-key' } },
  { id: 'boat', name: 'boat', label: 'boat', fields: { 'API key': 'fixture-key' } },
  { id: 'instabox', name: 'InstaBox', label: 'InstaBox', fields: { 'API key': 'fixture-key' } },
] satisfies { id: string; name: string; label: string; fields: Record<string, string>; noStop?: boolean }[];

for (const provider of manualProviders) {
  test(`${provider.name} manual machines are reachable on Linux and respect their power capabilities`, async ({ page }) => {
    await open(page, 'Linux x86_64');
    await expect(panel(page).getByText(/Local VMs need a Mac with Apple silicon/)).toBeVisible();
    await expect(panel(page).getByRole('button', { name: 'Set up VMs' })).toHaveCount(0);
    await connect(page, provider.name, provider.fields as Record<string, string>);
    expect(await calls(page, 'cloud_create')).toEqual([]);
    expect(await calls(page, 'cloud_power')).toEqual([]);
    await panel(page).locator(`[data-vm-create="cloud:${provider.id}"]`).click();
    await expect(panel(page).getByText(/has no screen/)).toBeVisible();
    const created = (await calls(page, 'cloud_create')).at(-1);
    expect(created.provider).toBe(provider.id);
    expect(created.name).toMatch(/^pandacode-[a-z0-9]+$/);
    await expect(control(page, 'Share project read-only at start')).toBeHidden();
    if ('noStop' in provider && provider.noStop) {
      await expect(control(page, 'Stop VM')).toBeDisabled();
      expect(await calls(page, 'cloud_power')).toEqual([]);
    } else {
      await control(page, 'Stop VM').click();
      await expect(panel(page).getByText(/is stopped/)).toBeVisible();
      await control(page, 'Start VM').click();
      await expect(panel(page).getByText(/has no screen/)).toBeVisible();
      expect((await calls(page, 'cloud_power')).map(call => call.action)).toEqual(['stop', 'start']);
    }
    await panel(page).getByRole('button', { name: 'Choose VM' }).click();
    await page.getByRole('menuitemradio', { name: `Delete ${created.name}`, exact: true }).click();
    await expect.poll(async () => (await calls(page, 'cloud_power')).at(-1)?.action).toBe('delete');
    await expect(panel(page).getByText('Create a machine', { exact: true })).toBeVisible();
    expect(await calls(page, 'vm_setup')).toEqual([]);
    expect(await calls(page, 'vm_create')).toEqual([]);
    expect(await calls(page, 'vm_desktop')).toEqual([]);
  });
}

test('InstaBox saves a validated image for future machines and requests a lease only when selected', async ({ page }) => {
  await open(page, 'Linux x86_64');
  await connect(page, 'InstaBox', { 'API key': 'fixture-key' });
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const settings = page.getByRole('region', { name: 'Settings', exact: true });
  await settings.getByRole('tab', { name: 'Cloud sandboxes', exact: true }).click();
  const images = settings.getByRole('combobox', { name: 'InstaBox machine image' });
  await expect(images).toBeEnabled();
  await images.selectOption('snapshot:ready-image');
  await expect(settings.locator('.cloud-image-status')).toContainText('Saved. New machines start from Prepared snapshot.');
  expect(await calls(page, 'cloud_instabox_set_image')).toEqual([{ image: 'snapshot:ready-image' }]);
  expect(await calls(page, 'cloud_create')).toEqual([]);
  await settings.getByRole('button', { name: 'Back to the workspace', exact: true }).click();
  await panel(page).locator('[data-vm-create="cloud:instabox"]').click();
  await expect(panel(page).getByText(/has no screen/)).toBeVisible();
  expect(await calls(page, 'cloud_keep_alive')).toEqual([]);
  await panel(page).getByRole('button', { name: 'Choose VM' }).click();
  await page.getByRole('menuitemradio', { name: 'Request 30-minute activity lease', exact: true }).click();
  await expect.poll(async () => (await calls(page, 'cloud_keep_alive')).at(-1)).toMatchObject({ id: expect.stringMatching(/^instabox:/), minutes: 30 });
});

test('Runloop labels the fixed six-hour activity renewal before requesting it', async ({ page }) => {
  await open(page, 'Linux x86_64');
  await connect(page, 'Runloop', { 'API key': 'fixture-key' });
  await panel(page).locator('[data-vm-create="cloud:runloop"]').click();
  await expect(panel(page).getByText(/has no screen/)).toBeVisible();
  await panel(page).getByRole('button', { name: 'Choose VM' }).click();
  await page.getByRole('menuitemradio', { name: 'Request 360-minute activity lease', exact: true }).click();
  await expect.poll(async () => (await calls(page, 'cloud_keep_alive')).at(-1)).toMatchObject({ id: expect.stringMatching(/^runloop:/), minutes: 360 });
});

test('a Vercel deploy connection needs a Sandbox project before offering machine creation', async ({ page }) => {
  await open(page, 'Linux x86_64');
  await connect(page, 'Vercel', { 'Access token': 'fixture-token', 'Team slug': 'fixture' });
  await expect(panel(page).locator('[data-vm-create="cloud:vercel"]')).toHaveCount(0);
  expect(await calls(page, 'cloud_create')).toEqual([]);
});

test('InstaBox ignores an old image list after disconnecting and connecting another account', async ({ page }) => {
  await open(page, 'Linux x86_64', `
    if (command === 'cloud_instabox_images' && window.__delayImages) {
      window.__delayImages = false;
      return new Promise(resolve => { window.__releaseOldImages = () => resolve({ images: [{ value: 'snapshot:old-account', label: 'Old account image' }], selected: 'snapshot:old-account' }); });
    }`);
  await page.evaluate(() => { (window as any).__delayImages = true; });
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const settings = page.getByRole('region', { name: 'Settings', exact: true });
  await settings.getByRole('tab', { name: 'Cloud sandboxes', exact: true }).click();
  const form = settings.getByRole('form', { name: 'Connect InstaBox' });
  await settings.getByRole('button', { name: 'Connect InstaBox', exact: true }).click();
  await form.getByLabel('InstaBox API key').fill('fixture-account-a');
  await form.locator('[type="submit"]').click();
  await expect.poll(() => page.evaluate(() => typeof (window as any).__releaseOldImages)).toBe('function');
  await settings.getByRole('button', { name: 'Disconnect InstaBox', exact: true }).click();
  await settings.getByRole('button', { name: 'Connect InstaBox', exact: true }).click();
  await form.getByLabel('InstaBox API key').fill('fixture-account-b');
  await form.locator('[type="submit"]').click();
  const images = settings.getByRole('combobox', { name: 'InstaBox machine image' });
  await expect(images).toBeEnabled();
  await expect(images).toHaveValue('template:agent-browser');
  await page.evaluate(async () => { (window as any).__releaseOldImages(); await new Promise<void>(resolve => requestAnimationFrame(() => resolve())); });
  await expect(images.locator('option')).toHaveText(['Agent browser', 'Prepared snapshot']);
  await expect(images).toHaveValue('template:agent-browser');
});

test('InstaBox prevents connection changes while an image save is pending', async ({ page }) => {
  await open(page, 'Linux x86_64', `
    if (command === 'cloud_instabox_set_image') {
      const result = previewCloud.setImage(args.image);
      return new Promise(resolve => { window.__releaseImageSave = () => resolve(result); });
    }`);
  await connect(page, 'InstaBox', { 'API key': 'fixture-key' });
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const settings = page.getByRole('region', { name: 'Settings', exact: true });
  await settings.getByRole('tab', { name: 'Cloud sandboxes', exact: true }).click();
  const images = settings.getByRole('combobox', { name: 'InstaBox machine image' });
  await expect(images).toBeEnabled();
  await images.selectOption('snapshot:ready-image');
  await expect(images).toBeDisabled();
  await expect(settings.getByRole('button', { name: 'Disconnect InstaBox', exact: true })).toBeDisabled();
  await expect(settings.getByRole('button', { name: 'Connect E2B', exact: true })).toBeDisabled();
  expect(await calls(page, 'cloud_disconnect')).toEqual([]);
  await page.evaluate(() => (window as any).__releaseImageSave());
  await expect(settings.getByRole('button', { name: 'Disconnect InstaBox', exact: true })).toBeEnabled();
  await expect(images).toHaveValue('snapshot:ready-image');
});
