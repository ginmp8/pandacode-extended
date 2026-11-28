import { escape } from './dom';
import type { CouncilConfig, CouncilCriterion, CouncilCriteriaSource, CouncilCriterionNativeEvidence } from './architect-types';

// These are existing native read-only evidence types, not local proof of execution.
export const FILE_CHECK_FIELDS = '<label>Native file check<select data-file-check-kind name="councilFileCheckKind">' +
  '<option value="">None (review evidence only)</option>' +
  '<option value="reviewed_file_sha256">File SHA-256</option>' +
  '<option value="reviewed_file_text_contains">File contains exact text</option>' +
  '<option value="reviewed_file_json_valid">Valid JSON file</option>' +
  '<option value="reviewed_file_toml_valid">Valid TOML file</option></select></label>' +
  '<div data-file-check-fields hidden><label>Repository-relative file<input data-file-check-path name="councilFileCheckPath" type="text" maxlength="240" autocomplete="off"></label>' +
  '<div data-file-check-sha-field hidden><label>Expected SHA-256<input data-file-check-sha name="councilFileCheckSha" type="text" maxlength="64" autocomplete="off"></label></div>' +
  '<div data-file-check-text-field hidden><label>Expected text<textarea data-file-check-text name="councilFileCheckText" maxlength="256" rows="2"></textarea></label></div>' +
  '<small>The native host must verify this check against the reviewed revision. No command execution is authorized.</small></div>';

export function fileCheckLens(kind: string, path: string): string {
  switch (kind) {
    case 'reviewed_file_sha256': return `File SHA-256: ${path}`;
    case 'reviewed_file_text_contains': return `File contains: ${path}`;
    case 'reviewed_file_json_valid': return `File valid JSON: ${path}`;
    case 'reviewed_file_toml_valid': return `File valid TOML: ${path}`;
    default: return '';
  }
}

function bindFileCheckFields(row: HTMLElement): void {
  const kind = row.querySelector<HTMLSelectElement>('[data-file-check-kind]');
  if (!kind) return; // Existing pinned checks retain their immutable editor path.
  const path = row.querySelector<HTMLInputElement>('[data-file-check-path]')!;
  const lens = row.querySelector<HTMLInputElement>('[data-criterion-lens]')!;
  const required = row.querySelector<HTMLInputElement>('[data-criterion-required]')!;
  const evidence = row.querySelector<HTMLInputElement>('[data-criterion-evidence]')!;
  let manual: { lens: string; required: boolean; evidence: boolean } | undefined;
  const sync = (): void => {
    const active = kind.value !== '';
    row.querySelector<HTMLElement>('[data-file-check-fields]')!.hidden = !active;
    row.querySelector<HTMLElement>('[data-file-check-sha-field]')!.hidden = kind.value !== 'reviewed_file_sha256';
    row.querySelector<HTMLElement>('[data-file-check-text-field]')!.hidden = kind.value !== 'reviewed_file_text_contains';
    if (active) {
      manual ??= { lens: lens.value, required: required.checked, evidence: evidence.checked };
      lens.value = fileCheckLens(kind.value, path.value);
      required.checked = true;
      evidence.checked = true;
    } else if (manual) {
      lens.value = manual.lens;
      required.checked = manual.required;
      evidence.checked = manual.evidence;
      manual = undefined;
    }
    lens.readOnly = active;
    required.disabled = active;
    evidence.disabled = active;
  };
  kind.addEventListener('change', sync);
  path.addEventListener('input', sync);
  sync();
}

export function readFileCheck(row: HTMLElement): CouncilCriterionNativeEvidence | undefined {
  const kind = row.querySelector<HTMLSelectElement>('[data-file-check-kind]')?.value;
  if (!kind) return undefined;
  const path = row.querySelector<HTMLInputElement>('[data-file-check-path]')!.value;
  const lens = fileCheckLens(kind, path);
  const bytes = (value: string): number => new TextEncoder().encode(value).length;
  if (!path || path !== path.trim() || bytes(path) > 240 || path.startsWith('/') || /[\\:\p{Cc}]/u.test(path)
    || path.split('/').some(part => !part || part === '.' || part === '..') || !lens || lens.length > 80) {
    throw new Error('Choose one repository-relative file with a valid review lens (at most 80 characters).');
  }
  if (kind === 'reviewed_file_sha256') {
    const expectedSha256 = row.querySelector<HTMLInputElement>('[data-file-check-sha]')!.value;
    if (!/^[a-f0-9]{64}$/.test(expectedSha256)) throw new Error('Enter a lowercase 64-character SHA-256 digest.');
    return { kind, path, expectedSha256 };
  }
  if (kind === 'reviewed_file_text_contains') {
    const expectedText = row.querySelector<HTMLTextAreaElement>('[data-file-check-text]')!.value;
    if (!expectedText || bytes(expectedText) > 256 || /\p{Cc}/u.test(expectedText.replace(/[\n\t]/g, ''))) {
      throw new Error('Enter a nonempty exact UTF-8 fragment of at most 256 bytes.');
    }
    return { kind, path, expectedText };
  }
  if (kind === 'reviewed_file_json_valid') return { kind, path };
  if (kind === 'reviewed_file_toml_valid' && path.endsWith('.toml')) return { kind, path };
  throw new Error('Choose a supported native file check and matching file type.');
}

/** Opt-in, task-scoped criteria within the existing Council modal; only native can accept a save. */
export function createCouncilCriteriaEditor(
  initial: CouncilConfig,
  current: () => CouncilConfig | null,
  save: (config: CouncilConfig) => void,
  error: (message: string) => void,
): HTMLElement {
  const root = document.createElement('section');
  root.className = 'council-criteria';
  root.setAttribute('aria-label', 'Task review criteria');
  root.innerHTML = '<button type="button" class="ds-btn council-criteria-toggle" aria-expanded="false" aria-controls="council-criteria-editor">Review criteria</button>' +
    '<div id="council-criteria-editor" class="council-criteria-editor" hidden>' +
      '<label>Criteria source<select data-criteria-source><option value="architect">Architect-defined</option><option value="user">User-defined</option><option value="hybrid">Hybrid</option></select></label>' +
      '<div class="council-criteria-fields" data-criteria-fields hidden><div data-criteria-rows class="council-criteria-rows"></div>' +
        '<button type="button" class="ds-btn" data-criteria-add>Add criterion</button></div>' +
      '<button type="button" class="ds-btn-primary" data-criteria-save>Apply review criteria</button></div>';
  const toggle = root.querySelector<HTMLButtonElement>('.council-criteria-toggle')!;
  const editor = root.querySelector<HTMLElement>('.council-criteria-editor')!;
  const source = root.querySelector<HTMLSelectElement>('[data-criteria-source]')!;
  const fields = root.querySelector<HTMLElement>('[data-criteria-fields]')!;
  const rows = root.querySelector<HTMLElement>('[data-criteria-rows]')!;
  source.value = initial.criteriaSource ?? 'architect';
  const add = (item: CouncilCriterion): HTMLElement => {
    const row = document.createElement('fieldset');
    row.className = 'council-criterion-row';
    row.setAttribute('data-criterion-row', '');
    row.innerHTML = '<legend>Criterion</legend>' +
      '<label>Criterion ID<input data-criterion-id type="text" maxlength="64" autocomplete="off" value="' + escape(item.id) + '"></label>' +
      '<label>Review lens<input data-criterion-lens type="text" maxlength="80" autocomplete="off" value="' + escape(item.lens) + '"></label>' +
      '<label>Minimum score<input data-criterion-target type="number" min="1" max="10" step="0.1" value="' + escape(String(item.target)) + '"></label>' +
      '<label><input data-criterion-required type="checkbox"' + (item.required ? ' checked' : '') + '> Required criterion</label>' +
      '<label><input data-criterion-evidence type="checkbox"' + (item.evidenceRequired ? ' checked' : '') + '> Require native evidence (blocks until verified)</label>' +
      (item.nativeEvidence ? '<small class="council-native-check">Native evidence check: ' + escape(item.nativeEvidence.kind) + ' (locked in this editor)</small>' : FILE_CHECK_FIELDS) +
      '<button data-criteria-remove type="button" class="ds-btn">Remove criterion</button>';
    rows.append(row);
    bindFileCheckFields(row);
    return row;
  };
  for (const item of initial.criteria?.length ? initial.criteria : [{ id: '', lens: '', target: initial.target, required: true }]) add(item);
  const update = (): void => { fields.hidden = source.value === 'architect'; };
  update();
  root.addEventListener('change', event => {
    event.stopPropagation(); // Never treat staged criterion input as a live Council switch.
    if (event.target === source) update();
  });
  root.addEventListener('click', event => {
    event.stopPropagation();
    const target = event.target as HTMLElement;
    if (target === toggle) {
      editor.hidden = !editor.hidden;
      toggle.setAttribute('aria-expanded', String(!editor.hidden));
      if (!editor.hidden) source.focus({ preventScroll: true });
      return;
    }
    if (target.closest('[data-criteria-add]')) {
      if (rows.children.length >= 8) { error('Council allows at most eight criterion lenses.'); return; }
      add({ id: '', lens: '', target: current()?.target ?? initial.target, required: true })
        .querySelector<HTMLInputElement>('[data-criterion-id]')?.focus({ preventScroll: true });
      return;
    }
    if (target.closest('[data-criteria-remove]')) {
      target.closest('[data-criterion-row]')?.remove();
      if (!rows.children.length) add({ id: '', lens: '', target: current()?.target ?? initial.target, required: true });
      rows.querySelector<HTMLInputElement>('[data-criterion-id]')?.focus({ preventScroll: true });
      return;
    }
    if (!target.closest('[data-criteria-save]')) return;
    const config = current();
    if (!config) { error('Turn Council on before applying task-scoped review criteria.'); return; }
    const mode = source.value as CouncilCriteriaSource;
    if (mode === 'architect') {
      if (config.criteria?.some(item => item.nativeEvidence)) {
        error('Native evidence checks cannot be removed in this editor. Keep the task criteria or explicitly reconfigure the native Council policy.');
        return;
      }
      save({ ...config, criteriaSource: undefined, criteria: undefined });
      return;
    }
    let criteria: CouncilCriterion[];
    try {
      criteria = [...rows.querySelectorAll<HTMLElement>('[data-criterion-row]')].map(row => {
        const nativeEvidence = readFileCheck(row);
        const lens = row.querySelector<HTMLInputElement>('[data-criterion-lens]')!.value.trim();
        const required = row.querySelector<HTMLInputElement>('[data-criterion-required]')!.checked;
        const evidenceRequired = row.querySelector<HTMLInputElement>('[data-criterion-evidence]')!.checked;
        if (nativeEvidence && (!('path' in nativeEvidence) || lens !== fileCheckLens(nativeEvidence.kind, nativeEvidence.path)
          || !required || !evidenceRequired)) throw new Error('A native file check requires its exact lens and mandatory evidence.');
        return {
          id: row.querySelector<HTMLInputElement>('[data-criterion-id]')!.value.trim(),
          lens, target: Number(row.querySelector<HTMLInputElement>('[data-criterion-target]')!.value),
          required, evidenceRequired, ...(nativeEvidence ? { nativeEvidence } : {}),
        };
      });
    } catch (cause) { error(cause instanceof Error ? cause.message : String(cause)); return; }
    const ids = criteria.map(c => c.id), lenses = criteria.map(c => c.lens.toLowerCase());
    if (criteria.length > 8 || !criteria.some(c => c.required)
      || criteria.some(c => !/^[a-zA-Z0-9_-]{1,64}$/.test(c.id) || !c.lens
        || c.lens.length > 80 || !Number.isFinite(c.target) || c.target < 1 || c.target > 10)
      || new Set(ids).size !== ids.length || new Set(lenses).size !== lenses.length) {
      error('Use unique criterion IDs and lenses, targets from 1 to 10, and at least one required criterion.');
      return;
    }
    // Applying the staged editor must not silently erase pinned native checks.
    // Preserve only checks with the same immutable ID/lens and mandatory gates;
    // an intentional policy replacement belongs to a separate native change.
    for (const existing of config.criteria ?? []) {
      if (!existing.nativeEvidence) continue;
      const next = criteria.find(item => item.id === existing.id);
      if (!next || next.nativeEvidence || next.lens !== existing.lens || !next.required || !next.evidenceRequired) {
        error('Native evidence checks cannot be changed or removed in this editor. Keep the criterion ID, lens and required evidence, or explicitly reconfigure the native Council policy.');
        return;
      }
      next.nativeEvidence = existing.nativeEvidence;
    }
    save({ ...config, criteriaSource: mode, criteria });
  });
  return root;
}
