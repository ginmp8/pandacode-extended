import { escape } from './dom';
import type { CouncilConfig, CouncilCriterion, CouncilCriteriaSource } from './architect-types';

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
      '<button data-criteria-remove type="button" class="ds-btn">Remove criterion</button>';
    rows.append(row);
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
      save({ ...config, criteriaSource: undefined, criteria: undefined });
      return;
    }
    const criteria = [...rows.querySelectorAll<HTMLElement>('[data-criterion-row]')].map(row => ({
      id: row.querySelector<HTMLInputElement>('[data-criterion-id]')!.value.trim(),
      lens: row.querySelector<HTMLInputElement>('[data-criterion-lens]')!.value.trim(),
      target: Number(row.querySelector<HTMLInputElement>('[data-criterion-target]')!.value),
      required: row.querySelector<HTMLInputElement>('[data-criterion-required]')!.checked,
      evidenceRequired: row.querySelector<HTMLInputElement>('[data-criterion-evidence]')!.checked,
    }));
    const ids = criteria.map(c => c.id), lenses = criteria.map(c => c.lens.toLowerCase());
    if (criteria.length > 8 || !criteria.some(c => c.required)
      || criteria.some(c => !/^[a-zA-Z0-9_-]{1,64}$/.test(c.id) || !c.lens
        || c.lens.length > 80 || !Number.isFinite(c.target) || c.target < 1 || c.target > 10)
      || new Set(ids).size !== ids.length || new Set(lenses).size !== lenses.length) {
      error('Use unique criterion IDs and lenses, targets from 1 to 10, and at least one required criterion.');
      return;
    }
    save({ ...config, criteriaSource: mode, criteria });
  });
  return root;
}
