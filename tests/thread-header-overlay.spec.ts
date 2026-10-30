import { expect, test, type Page } from '@playwright/test';

const box = (page: Page, selector: string) => page.locator(selector).first().evaluate(node => node.getBoundingClientRect().toJSON() as DOMRect);

async function openThread(page: Page, width = 1200): Promise<void> {
  // Short enough that the sample thread scrolls.
  await page.setViewportSize({ width, height: 520 });
  await page.addInitScript(() => localStorage.setItem('pandacode.wallpapers', 'false'));
  await page.goto('/');
  await page.locator('.task-item[data-id="session-1"]').evaluate(node => (node as HTMLElement).click());
  await expect(page.locator('#transcript .message').first()).toBeVisible();
}

test.describe('thread header overlay', () => {
  for (const width of [1200, 900, 620]) {
    test(`the transcript scrolls under the bare header with no fade at ${width}px`, async ({ page }) => {
      await openThread(page, width);
      const header = page.locator('.workspace-header');
      // No top bar: the header paints nothing and the transcript has no top fade.
      const style = await header.evaluate(node => ({ position: getComputedStyle(node).position, band: getComputedStyle(node, '::before').content, fill: getComputedStyle(node).backgroundColor, mask: (s => s.maskImage || s.getPropertyValue('-webkit-mask-image'))(getComputedStyle(document.querySelector('#transcript')!)) }));
      expect(style.position).toBe('relative');
      expect(style.band).toBe('none');
      expect(style.fill).toBe('rgba(0, 0, 0, 0)');
      expect(style.mask).toBe('none');
      await expect(page.locator('#breadcrumb')).toBeHidden();

      const transcript = page.locator('#transcript');
      const top = (await box(page, '#transcript')).top;
      // The transcript starts at the window's top edge, under the header, and its first message
      // sits just below the mask's fade (fully opaque at rest): flush with the top unless the
      // collapsed sidebar's buttons sit over the pane.
      expect(top).toBeLessThanOrEqual(1);
      const fade = await transcript.evaluate(node => parseFloat(getComputedStyle(node).getPropertyValue('--header-fade-under')) || 0);
      const collapsed = await page.locator('.workspace').evaluate(node => node.classList.contains('sidebar-collapsed'));
      expect(fade).toBeGreaterThan(0);
      await transcript.evaluate(node => { node.scrollTop = 0; });
      const first = (await box(page, '#transcript .message')).top - top;
      expect(first).toBeGreaterThanOrEqual(fade);
      expect(first).toBeLessThanOrEqual(collapsed ? 64 : 20);

      await page.locator('#stream-region').evaluate(node => { node.style.minHeight = '900px'; });
      await transcript.evaluate(node => { node.scrollTop = 90; });
      expect(await transcript.evaluate(node => node.scrollTop)).toBeGreaterThan(0);
      expect((await box(page, '#transcript .message')).top - top).toBeLessThan(first);

      // Scrolled to the end, the last row sits inside the transcript, above the composer.
      await transcript.evaluate(node => { node.scrollTop = node.scrollHeight; });
      const end = await transcript.evaluate(node => ({ last: node.lastElementChild!.previousElementSibling!.getBoundingClientRect().bottom, bottom: node.getBoundingClientRect().bottom }));
      expect(end.last).toBeLessThanOrEqual(end.bottom);
    });
  }

  test('the team lives in the dock, so nothing floats over the top of the thread', async ({ page }) => {
    await openThread(page);
    const transcript = page.locator('#transcript');
    await transcript.evaluate(node => { node.scrollTop = 0; });
    const before = (await box(page, '#transcript .message')).top;
    await page.getByRole('button', { name: 'Team', exact: true }).click();
    await page.getByRole('menuitemradio', { name: 'Balanced team', exact: true }).click();
    const agents = page.locator('#action-dock #agents-toggle');
    await expect(agents).toBeVisible();
    await expect(page.locator('.architect-trigger')).toBeHidden();
    // No room is reserved for the old floating bars.
    await transcript.evaluate(node => { node.scrollTop = 0; });
    expect((await box(page, '#transcript .message')).top).toBe(before);
    await agents.click();
    const flyout = page.locator('#agents-flyout');
    await expect(flyout.getByRole('dialog', { name: 'Workers', exact: true })).toBeVisible();
    // The panel opens left of the dock, beside the Agents item, and never covers the dock.
    const panel = await box(page, '#agents-flyout'), dock = await box(page, '#action-dock'), item = await box(page, '#agents-toggle');
    expect(panel.right).toBeLessThanOrEqual(dock.left);
    expect(Math.abs(panel.top - item.top)).toBeLessThanOrEqual(2);
    expect(panel.bottom).toBeLessThanOrEqual(520);
  });

  test('find navigation releases bottom-follow before streaming content grows', async ({ page }) => {
    await openThread(page);
    // Stabilize the preview before adding a unique match above the streaming region.
    await page.waitForTimeout(400);
    const transcript = page.locator('#transcript');
    const stream = page.locator('#stream-region');
    await stream.evaluate(node => { node.style.minHeight = '1200px'; });
    await page.locator('#transcript article.user-message .message-content').first()
      .evaluate(node => node.append(document.createTextNode(' unique-stream-reader-anchor ')));
    await transcript.evaluate(node => { node.scrollTop = node.scrollHeight; });
    await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));

    await page.keyboard.press('ControlOrMeta+f');
    const find = page.getByRole('search');
    await find.getByRole('searchbox', { name: 'Find in thread', exact: true }).fill('unique-stream-reader-anchor');
    await expect(find.locator('.thread-find-count')).toHaveText('1 of 1');
    const position = () => page.evaluate(() => {
      const highlights = (CSS as any).highlights.get('thread-find-current');
      const range = [...highlights][0] as Range;
      const pane = document.querySelector<HTMLElement>('#transcript')!;
      const rect = range.getBoundingClientRect();
      return {
        matchTop: rect.top,
        paneTop: pane.getBoundingClientRect().top,
        paneBottom: pane.getBoundingClientRect().bottom,
        gap: pane.scrollHeight - pane.scrollTop - pane.clientHeight,
      };
    });
    const before = await position();
    expect(before.matchTop).toBeGreaterThanOrEqual(before.paneTop);
    expect(before.matchTop).toBeLessThan(before.paneBottom);
    expect(before.gap).toBeGreaterThan(200);

    // Streaming growth triggers the real transcript ResizeObserver. It must not snap the
    // reader back to the bottom after find moved the viewport to an earlier match.
    await stream.evaluate(node => { node.style.minHeight = '1800px'; });
    await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    const after = await position();
    expect(Math.abs(after.matchTop - before.matchTop)).toBeLessThan(16);
    expect(after.gap).toBeGreaterThan(400);
  });

  test('finding a visible match releases bottom-follow intent', async ({ page }) => {
    await page.goto('/');
    // Real find widget with a self-contained, visible match and no scrolling required.
    const result = await page.evaluate(async () => {
      const { createThreadFind } = await import('/src/thread-find.ts');
      const pane = document.createElement('div');
      pane.style.cssText = 'position:fixed;top:0;left:0;width:360px;height:200px;z-index:9999';
      const transcript = document.createElement('section');
      transcript.style.cssText = 'height:200px;overflow:auto';
      transcript.innerHTML = '<div style="height:100px"></div><article class="message"><div class="message-content">visible-target</div></article>';
      pane.append(transcript);
      document.body.append(pane);
      let releases = 0;
      const find = createThreadFind({
        pane: () => pane,
        transcript: () => transcript,
        restoreFocus: () => {},
        beforeReveal: () => { releases++; },
      });
      find.open();
      const input = pane.querySelector<HTMLInputElement>('input')!;
      input.value = 'visible-target';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      const range = [...(CSS as any).highlights.get('thread-find-current')][0] as Range;
      const rect = range.getBoundingClientRect();
      const visible = rect.top > 50 && rect.bottom < 200;
      find.close();
      pane.remove();
      return { releases, visible };
    });
    expect(result).toEqual({ releases: 1, visible: true });
  });

  test('find in thread lands every match below the fade and the find bar', async ({ page }) => {
    await openThread(page);
    // Let the thread's refresh redraw land first; it restores the scroll position it remembered.
    await page.waitForTimeout(400);
    const transcript = page.locator('#transcript');
    // Room to scroll, then put the first "search" match just under the header, inside the fade.
    await page.locator('#stream-region').evaluate(node => { node.style.minHeight = '900px'; });
    const fadeBottom = (await box(page, '#transcript')).top + await transcript.evaluate(node => parseFloat(getComputedStyle(node).getPropertyValue('--header-fade-under')) || 0);
    // Retry until the position holds: the taller stream region can still pull the transcript to
    // the bottom for a frame or two after the first scroll.
    await expect.poll(async () => {
      await transcript.evaluate((node, y) => { node.scrollTop += node.querySelector('.user-message')!.getBoundingClientRect().top - y; }, fadeBottom - 10);
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      return transcript.evaluate(node => Math.round(node.querySelector('.user-message')!.getBoundingClientRect().top));
    }).toBe(Math.round(fadeBottom - 10));
    await page.keyboard.press('ControlOrMeta+f');
    const find = page.getByRole('search');
    const input = find.getByRole('searchbox', { name: 'Find in thread', exact: true });
    await input.fill('search');
    const count = find.locator('.thread-find-count');
    await expect(count).toHaveText(/^\d+ of \d+$/);
    const total = Number((await count.textContent())!.split(' of ')[1]);
    const current = () => page.evaluate(() => { const [range] = [...(CSS as any).highlights.get('thread-find-current')] as Range[]; return range.getBoundingClientRect().toJSON() as DOMRect; });
    const bar = (await find.boundingBox())!;
    const bottom = (await box(page, '#transcript')).bottom;
    // The fresh search skips the faded match; stepping back to it, then through every match, must
    // bring each fully into view: below the fade and never under the find bar. At the very top of
    // the thread a match can sit above the bar, since the first message is flush with the top.
    for (const [index, key] of ['', 'Shift+Enter', ...Array(total).fill('Enter')].entries()) {
      if (key) await input.press(key);
      const rect = await current();
      expect(rect.top, `step ${index}`).toBeGreaterThanOrEqual(fadeBottom);
      expect(rect.bottom, `step ${index}`).toBeLessThanOrEqual(bottom);
      const underBar = rect.left < bar.x + bar.width && bar.x < rect.right && rect.top < bar.y + bar.height && bar.y < rect.bottom;
      expect(underBar, `step ${index}`).toBe(false);
    }
  });
});
