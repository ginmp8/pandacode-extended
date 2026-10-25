import { expect, test, type Page } from '@playwright/test';

async function failingNativeReads(page: Page, delayed = false) {
  await page.route('**/src/preview.ts*', async route => {
    const response = await route.fetch(), source = await response.text();
    const marker = /async function invoke\(command, args\) \{/;
    expect(source).toMatch(marker);
    await route.fulfill({ response, body: source.replace(marker, `async function invoke(command, args) {
      (window.__historicalDropCalls ??= []).push(command);
      if (command === 'describe_folder') throw new Error('not a folder');
      if (command === 'read_document') {
        ${delayed ? "await new Promise(resolve => window.__releaseHistoricalDrop = resolve);" : ''}
        throw new Error('Unsupported file format');
      }
    `) });
  });
  await page.goto('/');
  await expect(page.locator('#prompt')).toBeEnabled();
}
async function drop(page: Page, paths: string[]) {
  await page.evaluate(paths => document.dispatchEvent(new CustomEvent('pandacode-preview-file-drop', { detail: { paths } })), paths);
}

test('unsupported native drops insert quoted paths without sending', async ({ page }) => {
  await failingNativeReads(page);
  await page.locator('#prompt').fill('Inspect');
  await drop(page, ['/tmp/large document.bin', '/tmp/another.bin']);
  await expect(page.locator('#prompt')).toHaveValue('Inspect\n"/tmp/large document.bin"\n/tmp/another.bin');
  await expect(page.locator('#attachments .prompt-chip')).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).__historicalDropCalls.includes('send_message'))).toBe(false);
});

test('a delayed failed drop cannot write into a newly selected task', async ({ page }) => {
  await failingNativeReads(page, true);
  await page.locator('#prompt').fill('Original draft');
  await drop(page, ['/tmp/late.bin']);
  await expect.poll(() => page.evaluate(() => typeof (window as any).__releaseHistoricalDrop)).toBe('function');
  await page.locator('.task-item[data-id="session-2"]').click();
  await page.locator('#prompt').fill('Other draft');
  await page.evaluate(() => (window as any).__releaseHistoricalDrop());
  await expect(page.locator('#toast')).toContainText('Unsupported file format');
  await expect(page.locator('#prompt')).toHaveValue('Other draft');
  await page.locator('.task-item[data-id="session-1"]').click();
  await expect(page.locator('#prompt')).toHaveValue('Original draft');
});

test('browser drops identify the filename without inventing a local path', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#prompt')).toBeEnabled();
  await page.evaluate(() => {
    const data = new DataTransfer(); data.items.add(new File([new Uint8Array([0, 1, 2])], 'archive.bin', { type: 'application/octet-stream' }));
    document.dispatchEvent(new DragEvent('drop', { dataTransfer: data, bubbles: true, cancelable: true }));
  });
  await expect(page.locator('#prompt')).toHaveValue('archive.bin (dropped file; its local path is unavailable)');
  await expect(page.locator('#attachments .prompt-chip')).toHaveCount(0);
});
