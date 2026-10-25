import createPurify from 'dompurify';
import type { HElement } from 'docx-preview';

// Word supplies styles and relationships. Render into a document with no browsing
// context, then sanitize before adopting any nodes into the application's view.
const STYLE_PROPERTIES = /^(?:color|background-color|font(?:-family|-size|-weight|-style)?|text-(?:align|decoration|indent|transform)|line-height|letter-spacing|white-space|word-break|overflow-wrap|vertical-align|display|width|height|min-width|max-width|min-height|max-height|margin(?:-(?:top|right|bottom|left))?|padding(?:-(?:top|right|bottom|left))?|border(?:-(?:top|right|bottom|left))?(?:-(?:width|style|color))?|border-collapse|border-spacing|box-sizing|page-break-(?:before|after|inside)|break-(?:before|after|inside)|list-style-type)$/;
export const inlineRaster = (url: string): boolean => url.length <= 12 * 1024 * 1024 && /^data:image\/(?:png|jpeg|webp|gif|bmp);base64,[a-z\d+/=\s]+$/i.test(url);

function safeStyle(style: CSSStyleDeclaration): string {
  const declarations: string[] = [];
  for (const property of Array.from(style)) {
    const value = style.getPropertyValue(property);
    if (STYLE_PROPERTIES.test(property) && value.length <= 256 && !/[\\@]|url\s*\(|var\s*\(|expression\s*\(/i.test(value)) declarations.push(`${property}:${value}`);
  }
  return declarations.join(';');
}

export function docxStaging() {
  const staging = document.implementation.createHTMLDocument('Document preview');
  let count = 0;
  function h(input: HElement | Node | string): Node {
    if (++count > 50_000) throw new Error('The Word document has too many elements to preview.');
    if (typeof input === 'string') return staging.createTextNode(input);
    if (input instanceof Node) return input;
    const { ns, tagName, className, style, children, ...props } = input;
    if (tagName === '#comment') return staging.createComment('');
    if (tagName === '#fragment') {
      const fragment = staging.createDocumentFragment();
      for (const child of children ?? []) fragment.append(h(child));
      return fragment;
    }
    const element = ns ? staging.createElementNS(ns, tagName) : staging.createElement(tagName);
    if (className) element.setAttribute('class', className);
    if (style) {
      if (typeof style === 'string') element.setAttribute('style', style);
      else Object.assign((element as HTMLElement).style, style);
    }
    for (const [key, value] of Object.entries(props)) {
      if (value === undefined) continue;
      if (key === 'textContent') element.textContent = String(value);
      else if (['href', 'src', 'id', 'title', 'alt', 'colSpan', 'rowSpan'].includes(key)) element.setAttribute(key.toLowerCase(), String(value));
    }
    for (const child of children ?? []) element.appendChild(h(child));
    return element;
  }

  const purify = createPurify(window);
  purify.addHook('afterSanitizeAttributes', node => {
    if (!(node instanceof Element)) return;
    if (node.hasAttribute('style')) {
      const safe = safeStyle((node as HTMLElement).style);
      if (safe) node.setAttribute('style', safe); else node.removeAttribute('style');
    }
    if (node.tagName === 'IMG' && !inlineRaster(node.getAttribute('src') ?? '')) node.removeAttribute('src');
    if (node.tagName === 'A') {
      const href = node.getAttribute('href') ?? '';
      if (!/^https?:\/\//i.test(href)) node.removeAttribute('href');
      else { node.setAttribute('target', '_blank'); node.setAttribute('rel', 'noopener noreferrer'); }
    }
  });

  function sanitize(nodes: Node[]): DocumentFragment {
    const result = document.createDocumentFragment();
    for (const node of nodes) {
      if (node.nodeName === 'STYLE') {
        // Parsing in the inert document does not fetch imports. Only ordinary
        // style rules and non-fetching declarations survive the copy.
        const input = staging.createElement('style'); input.textContent = node.textContent;
        staging.head.append(input);
        const rules: string[] = [];
        for (const rule of Array.from(input.sheet?.cssRules ?? [])) {
          if (rule.type !== CSSRule.STYLE_RULE) continue;
          const style = rule as CSSStyleRule;
          if (style.selectorText.length > 2048 || /[\\@]/.test(style.selectorText)) continue;
          const declarations = safeStyle(style.style);
          if (declarations) rules.push(`${style.selectorText}{${declarations}}`);
        }
        input.remove();
        const safe = document.createElement('style'); safe.textContent = rules.join('\n'); result.append(safe);
      } else {
        const holder = staging.createElement('div'); holder.append(node);
        const clean = purify.sanitize(holder, { IN_PLACE: true, USE_PROFILES: { html: true }, ALLOW_DATA_ATTR: false, FORBID_TAGS: ['style', 'script', 'iframe', 'object', 'embed', 'form', 'input', 'button', 'textarea', 'select', 'video', 'audio', 'link', 'meta', 'source'], FORBID_ATTR: ['srcset', 'background', 'download', 'id', 'name'] });
        result.append(...clean.childNodes);
      }
    }
    return result;
  }
  return { h, sanitize };
}
