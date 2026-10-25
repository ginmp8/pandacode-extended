import { expect, test, type Page } from '@playwright/test';
import { openInspectorTab } from './inspector-tab-helper';

test('Markdown never decodes unchecked inline images and bounds workspace image requests', async ({ page }) => {
  await page.goto('/');
  const result = await page.evaluate(async () => {
    // A plausible oversized GIF header must never be sent to a browser decoder.
    const oversized = 'data:image/gif;base64,' + btoa('GIF89a' + String.fromCharCode(255, 255, 255, 255));
    const reader = document.createElement('div'); document.body.append(reader);
    const paths: string[] = [];
    const { renderDocument } = await import('/src/document-viewer.ts');
    renderDocument(reader, 'README.md', '<div>' + `<img src="${oversized}" alt="unverified inline">`.repeat(40)
      + Array.from({ length: 40 }, (_, index) => `<img src="image-${index}.png" alt="workspace">`).join('') + '</div>', async path => {
        paths.push(path); return 'data:image/png;base64,AA==';
      });
    await new Promise<void>(resolve => { const check = () => paths.length ? resolve() : setTimeout(check, 10); check(); });
    return { requests: paths.length, unchecked: reader.querySelectorAll(`img[src="${oversized}"]`).length, text: reader.textContent };
  });
  expect(result.requests).toBe(32);
  expect(result.unchecked).toBe(0);
  expect(result.text).toContain('unverified inline');
});

async function files(page: Page) {
  await page.goto('/');
  await openInspectorTab(page, 'Files');
  await page.getByRole('button', { name: 'Files', exact: true }).click();
}

test('project files open beside Conversation and keep the transcript mounted', async ({ page }) => {
  await files(page);
  const before = await page.locator('#transcript').textContent();
  await page.locator('[data-action="open-file"][data-path="README.md"]').first().click();
  const tabs = page.getByRole('tablist', { name: 'Open files' });
  await expect(tabs.getByRole('tab', { name: 'README.md', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('#file-view .doc-reader h1')).toHaveText('Orbit');
  await expect(page.locator('#transcript')).toHaveAttribute('inert', '');
  await tabs.getByRole('tab', { name: 'Conversation', exact: true }).click();
  await expect(page.locator('#file-view')).toBeHidden();
  await expect(page.locator('#transcript')).not.toHaveAttribute('inert', '');
  expect(await page.locator('#transcript').textContent()).toBe(before);
});

test('file tabs keep task ownership and support keyboard close', async ({ page }) => {
  await files(page);
  const task = await page.locator('#prompt').getAttribute('data-session-id');
  await page.locator('[data-action="open-file"][data-path="README.md"]').first().click();
  const readme = page.getByRole('tablist', { name: 'Open files' }).getByRole('tab', { name: 'README.md', exact: true });
  await expect(readme).toBeVisible();
  const other = page.locator(`.task-item:not([data-id="${task}"])`).first();
  await other.click();
  await expect(page.locator('#file-tabs')).toBeHidden();
  await page.locator(`.task-item[data-id="${task}"]`).click();
  await expect(readme).toHaveAttribute('aria-selected', 'true');
  await readme.focus();
  await page.keyboard.press('Delete');
  await expect(page.locator('#file-tabs')).toBeHidden();
  await expect(page.locator('#prompt')).toBeFocused();
});

test('a late file read cannot draw over another task or resurrect a closed tab', async ({ page }) => {
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch(), source = await response.text();
    const marker = /async function invoke\(command, args\) \{/;
    expect(source).toMatch(marker);
    await route.fulfill({ response, body: source.replace(marker, `async function invoke(command, args) {
      if (command === 'read_file' && args.path === 'README.md') return await new Promise(resolve => window.__releaseHistoricalFile = () => resolve('# Late original task file'));
    `) });
  });
  await files(page);
  const original = await page.locator('#prompt').getAttribute('data-session-id');
  await page.locator('[data-action="open-file"][data-path="README.md"]').first().click();
  await expect(page.locator('#file-view')).toContainText('Loading file');
  await page.getByRole('button', { name: 'Close README.md', exact: true }).click();
  await page.locator(`.task-item:not([data-id="${original}"])`).first().click();
  await page.evaluate(() => (window as any).__releaseHistoricalFile());
  await expect(page.locator('#file-tabs')).toBeHidden();
  await expect(page.locator('#file-view')).not.toContainText('Late original task file');
  await page.locator(`.task-item[data-id="${original}"]`).click();
  await expect(page.locator('#file-tabs')).toBeHidden();
});

test('Word staging strips active content and fetching styles before mounting', async ({ page }) => {
  const outbound: string[] = [];
  page.on('request', request => { if (request.url().includes('document-egress.invalid')) outbound.push(request.url()); });
  await page.goto('/');
  const result = await page.evaluate(async () => {
    const { docxStaging } = await import('/src/office-safety.ts');
    const stage = docxStaging();
    const host = document.createElement('div'); document.body.append(host);
    const shadow = host.attachShadow({ mode: 'open' });
    const image = stage.h({ tagName: 'img', src: 'https://document-egress.invalid/image', onerror: 'window.badDocument = true' });
    const link = stage.h({ tagName: 'a', href: 'javascript:window.badDocument=true', children: ['Unsafe link'] });
    const style = stage.h({ tagName: 'style', textContent: '@import "https://document-egress.invalid/css";.docx{background-image:url(https://document-egress.invalid/background);position:fixed;color:red}' });
    shadow.append(stage.sanitize([image, link, style]));
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    return { html: shadow.innerHTML, ran: Boolean((window as any).badDocument) };
  });
  expect(result.ran).toBe(false);
  expect(result.html).not.toMatch(/document-egress|javascript:|onerror|position:fixed|@import/);
  expect(outbound).toEqual([]);
});
