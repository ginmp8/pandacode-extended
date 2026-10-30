import test from 'node:test';
import assert from 'node:assert/strict';
import { avatarPicker, botAvatarHtml, PANDAS, pandaSvg, safeBotColor } from '../src/bot-avatars.ts';

test('ten pandas, each its own color, each a compact id-free SVG', () => {
  assert.deepEqual(PANDAS.map(p => p.id), Array.from({ length: 10 }, (_, i) => `panda:${i}`));
  assert.equal(new Set(PANDAS.map(p => p.color)).size, 10);
  for (const p of PANDAS) {
    const svg = pandaSvg(p.id);
    assert.match(svg, /^<svg viewBox="0 0 64 64"/);
    assert.ok(svg.includes(p.color) && !/\sid=/.test(svg));
  }
  assert.equal(pandaSvg('panda:10'), '');
});

test('avatars render pandas, uploaded images, legacy glyphs and escaped initials', () => {
  assert.match(botAvatarHtml('panda:3', 'Ana', 'x'), /^<span class="bot-avatar has-art x" aria-hidden="true"><svg/);
  const png = 'data:image/png;base64,iVBORw0KGgo=';
  assert.ok(botAvatarHtml(png, 'Ana').includes(`<img alt="" src="${png}">`));
  assert.ok(!botAvatarHtml('data:image/png;base64,"><script>', 'Ana').includes('<script>'));
  assert.equal(botAvatarHtml('🐼', 'Ana'), '<span class="bot-avatar" aria-hidden="true">🐼</span>');
  assert.equal(botAvatarHtml('', '<b>'), '<span class="bot-avatar" aria-hidden="true">&#60;</span>');
});

test('the face picker marks one of six faces and shows an uploaded image as its own swatch', () => {
  const html = avatarPicker('panda:2');
  assert.equal(html.match(/data-avatar-choice="panda:/g).length, 6);
  assert.ok(html.includes('data-avatar-choice="panda:2" aria-pressed="true" aria-label="Wink panda"'));
  assert.ok(html.includes('role="group" aria-label="Face"') && html.includes('data-avatar-upload'));
  assert.ok(!html.includes('data-avatar-choice="custom"'));
  assert.ok(avatarPicker('data:image/png;base64,AAAA').includes('data-avatar-choice="custom" aria-pressed="true"'));
});

// Hardened additions: untrusted avatar values never become executable markup or huge images.
test('blocks oversized and unsupported image URL content in avatar markup', () => {
  const overlong = 'data:image/png;base64,' + 'A'.repeat(300_001);
  assert.ok(!botAvatarHtml(overlong, 'Ana').includes('<img'));
  assert.ok(!avatarPicker(overlong).includes('data-avatar-choice="custom"'));
  assert.ok(!botAvatarHtml('data:image/svg+xml;base64,PHN2Zz4=', 'Ana').includes('<img'));
  assert.ok(!botAvatarHtml('data:image/jpeg;base64,AA==', 'Ana').includes('<img'));
});

test('shared-channel bot colors reject unsafe CSS declarations', () => {
  assert.equal(safeBotColor('#aBc123'), '#aBc123');
  assert.equal(safeBotColor(undefined), '');
  assert.equal(safeBotColor(''), '');
  for (const color of ['red', '#fff', 'var(--x)', '#123456;background-image:url(https://example.org/x)', '#123456\" onmouseover=\"x', 'rgb(1,2,3)']) {
    assert.equal(safeBotColor(color), '');
    assert.ok(!botAvatarHtml('panda:0', 'Ana', '', color).includes('style="--panda-bg:'));
  }
});
