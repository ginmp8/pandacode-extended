import { escape } from './dom';

/** Local-only wizard state. It cannot grant permissions or run a model. */
export interface TeamBuilderBot {
  id: string;
  name: string;
  paused: boolean;
  projectId?: string;
  provider?: string;
  model?: string;
  mode?: string;
  skills?: string[];
}
export interface TeamBuilderInput {
  name: string;
  coordinatorBotId: string;
  workerBotId: string;
  workerBotIds: string[];
  reviewerBotIds: string[];
  councilRequired: boolean;
  executionMode: 'approval' | 'full_auto';
}

export class GuidedTeamBuilder {
  private visible = false;
  private step = 1;
  private error = '';
  private template = '';
  private name = '';
  private coordinatorId = '';
  private primaryWorkerId = '';
  private workers: string[] = [];
  private reviewers: string[] = [];
  private councilRequired = false;
  private executionMode: 'approval' | 'full_auto' = 'approval';

  get isOpen(): boolean { return this.visible; }

  reset(): void {
    this.visible = false;
    this.step = 1;
    this.error = '';
    this.template = '';
    this.name = '';
    this.coordinatorId = '';
    this.primaryWorkerId = '';
    this.workers = [];
    this.reviewers = [];
    this.councilRequired = false;
    this.executionMode = 'approval';
  }

  private allIds(): string[] {
    return [this.coordinatorId, this.primaryWorkerId, ...this.workers, ...this.reviewers]
      .filter(Boolean);
  }

  private problem(bots: TeamBuilderBot[], throughStep: number): string | null {
    if (throughStep >= 1 && (!this.name.trim() || this.name.trim().length > 120)) {
      return 'Choose a Team name between 1 and 120 characters.';
    }
    if (throughStep < 2) return null;
    if (!this.coordinatorId || !this.primaryWorkerId) {
      return 'Choose an existing Coordinator and primary Worker.';
    }
    if (this.workers.length > 15 || this.reviewers.length > 8) {
      return 'A Team supports at most 16 Workers and eight Reviewers.';
    }
    const members = this.allIds();
    if (new Set(members).size !== members.length) {
      return 'Each existing bot must have exactly one role in this Team.';
    }
    if (members.some(id => !bots.some(bot => bot.id === id && !bot.paused))) {
      return 'Remove unavailable or paused bots before saving.';
    }
    const rootProject = bots.find(bot => bot.id === this.primaryWorkerId)?.projectId;
    if (this.workers.some(id => bots.find(bot => bot.id === id)?.projectId !== rootProject)) {
      return 'All Workers must share the primary Worker project scope.';
    }
    if (throughStep >= 4 && this.councilRequired && this.reviewers.length < 2) {
      return 'Required native Council needs at least two selected Reviewers.';
    }
    return null;
  }

  validatedInput(bots: TeamBuilderBot[]): TeamBuilderInput | null {
    if (!this.visible || this.step !== 5) return null;
    this.error = this.problem(bots, 4) ?? '';
    if (this.error) return null;
    return {
      name: this.name.trim(),
      coordinatorBotId: this.coordinatorId,
      workerBotId: this.primaryWorkerId,
      workerBotIds: [this.primaryWorkerId, ...this.workers],
      reviewerBotIds: [...this.reviewers],
      councilRequired: this.councilRequired,
      executionMode: this.executionMode,
    };
  }

  handleInput(target: Element): boolean {
    if (!this.visible || !(target instanceof HTMLInputElement) ||
        target.name !== 'builderName' || !target.closest('form[data-sa-form="guided-builder"]')) {
      return false;
    }
    this.name = target.value;
    this.error = '';
    return true;
  }

  handleChange(target: Element): boolean {
    if (!this.visible ||
        !(target instanceof HTMLInputElement || target instanceof HTMLSelectElement) ||
        !target.closest('form[data-sa-form="guided-builder"]')) {
      return false;
    }
    switch (target.name) {
      case 'builderTemplate': {
        const old = this.template;
        this.template = target.value;
        if (!this.name || this.name === old) this.name = this.template;
        break;
      }
      case 'builderCoordinator': this.coordinatorId = target.value; break;
      case 'builderWorker': this.primaryWorkerId = target.value; break;
      case 'builderWorkers': {
        if (!(target instanceof HTMLSelectElement)) return false;
        const ids = Array.from(target.selectedOptions, item => item.value);
        this.workers = [...this.workers.filter(id => ids.includes(id)),
          ...ids.filter(id => !this.workers.includes(id))];
        break;
      }
      case 'builderReviewers':
        if (!(target instanceof HTMLSelectElement)) return false;
        this.reviewers = Array.from(target.selectedOptions, item => item.value);
        break;
      case 'builderExecutionMode':
        if (!(target instanceof HTMLInputElement) ||
            !['approval','full_auto'].includes(target.value)) return false;
        this.executionMode = target.value as 'approval' | 'full_auto';
        this.councilRequired = this.executionMode === 'full_auto';
        break;
      default: return false;
    }
    this.error = '';
    return true;
  }

  handleClick(target: HTMLButtonElement, bots: TeamBuilderBot[]): boolean {
    if (target.hasAttribute('data-sa-builder-open')) {
      this.visible = true;
      this.step = 1;
      this.error = '';
      return true;
    }
    if (!this.visible) return false;
    if (target.hasAttribute('data-sa-builder-step')) {
      const requestedStep = Number(target.dataset.saBuilderStep);
      if (Number.isInteger(requestedStep) && requestedStep >= 1 && requestedStep < this.step) {
        this.step = requestedStep;
        this.error = '';
      }
      return true;
    }
    if (target.hasAttribute('data-sa-builder-reviewers')) {
      this.step = 2;
      this.error = '';
      return true;
    }
    if (target.hasAttribute('data-sa-builder-cancel')) {
      this.reset();
      return true;
    }
    if (target.dataset.saBuilderMove) {
      const index = this.workers.indexOf(target.dataset.saBuilderMove);
      const next = index + (target.dataset.saBuilderDirection === 'up' ? -1 : 1);
      if (index >= 0 && next >= 0 && next < this.workers.length) {
        [this.workers[index], this.workers[next]] = [this.workers[next], this.workers[index]];
      }
      return true;
    }
    if (target.hasAttribute('data-sa-builder-next')) {
      const problem = this.problem(bots, this.step);
      if (problem) this.error = problem;
      else { this.step = Math.min(5, this.step + 1); this.error = ''; }
      return true;
    }
    if (target.hasAttribute('data-sa-builder-prev')) {
      this.step = Math.max(1, this.step - 1);
      this.error = '';
      return true;
    }
    return false;
  }

  render(bots: TeamBuilderBot[]): string {
    const safe = (input: unknown) => escape(String(input ?? ''));
    if (!this.visible) return '';
    const selectedBot = (id: string) => bots.find(bot => bot.id === id);
    const name = (id: string) => selectedBot(id)?.name ?? 'Missing bot';
    const selectOptions = (selectedId: string, exclude: string[] = []) =>
      '<option value="">Choose existing bot</option>' +
      bots.filter(bot => !exclude.includes(bot.id))
        .map(bot => '<option value="' + safe(bot.id) + '"' +
          (selectedId === bot.id ? ' selected' : '') +
          (bot.paused ? ' disabled' : '') + '>' +
          safe(bot.name) + (bot.paused ? ' (paused)' : '') + '</option>').join('');
    const multiselectOptions = (
      selectedIds: string[], exclude: string[], workerProjectId?: string,
    ) => bots.filter(bot => !exclude.includes(bot.id))
      .map(bot => {
        const crossProject = !!workerProjectId && bot.projectId !== workerProjectId;
        return '<option value="' + safe(bot.id) + '"' +
          (selectedIds.includes(bot.id) ? ' selected' : '') +
          (bot.paused || crossProject ? ' disabled' : '') + '>' +
          safe(bot.name) + (bot.paused ? ' (paused)' :
            crossProject ? ' (different Worker project)' : '') + '</option>';
      }).join('');
    const steps = ['Purpose', 'Team roles', 'Models and Skills',
      'Execution and review', 'Preflight and Save'];
    let content = '';
    if (this.step === 1) {
      content = '<label>Scenario template<select name="builderTemplate">' +
        '<option value="">Custom</option>' +
        ['Software Development', 'Issue Resolution'].map(value =>
          '<option value="' + value + '"' + (this.template === value ? ' selected' : '') +
          '>' + value + '</option>').join('') + '</select></label>' +
        '<label>Team name<input name="builderName" maxlength="120" required value="' +
        safe(this.name) + '" placeholder="Scenario-specific Team"></label>' +
        '<p>One saved Team can accept multiple independent Demands. The primary Worker ' +
        'determines the execution project. GitHub sources are configured after creation.</p>';
    } else if (this.step === 2) {
      content = '<label>Coordinator<select name="builderCoordinator">' +
        selectOptions(this.coordinatorId) + '</select></label>' +
        '<label>Primary Worker<select name="builderWorker">' +
        selectOptions(this.primaryWorkerId) + '</select></label>' +
        '<label>Additional Workers (up to 15)<select name="builderWorkers" multiple size="5">' +
        multiselectOptions(this.workers, [this.coordinatorId, this.primaryWorkerId, ...this.reviewers],
          bots.find(bot => bot.id === this.primaryWorkerId)?.projectId) +
        '</select></label>' +
        '<ol aria-label="Configured Worker order">' +
        this.workers.map((id, index) =>
          '<li>' + safe(name(id)) +
          ' <button type="button" data-sa-builder-move="' + safe(id) +
          '" data-sa-builder-direction="up"' + (index === 0 ? ' disabled' : '') + '>Up</button>' +
          ' <button type="button" data-sa-builder-move="' + safe(id) +
          '" data-sa-builder-direction="down"' +
          (index === this.workers.length - 1 ? ' disabled' : '') + '>Down</button></li>').join('') +
        '</ol>' +
        '<label>Optional Council Reviewers (up to 8)<select name="builderReviewers" multiple size="5">' +
        multiselectOptions(this.reviewers, [this.coordinatorId,this.primaryWorkerId,...this.workers]) +
        '</select></label>' +
        '<p>Native validation enforces active bot identities and one project for all Workers.</p>';
    } else if (this.step === 3) {
      content = '<p>These are existing bot configurations, not new credentials or grants. ' +
        'Installed, harness-eligible, invoked and evidence-verified are distinct states.</p>' +
        '<ol aria-label="Configured Models and Skills">' +
        this.allIds().map(id => {
          const bot = selectedBot(id);
          return '<li>' + safe(name(id)) + ' · ' +
            safe(bot?.provider ?? 'Unknown harness') + ' / ' +
            safe(bot?.model ?? 'Unknown model') +
            ' · Mode ' + safe(bot?.mode ?? 'Unknown') +
            ' · assigned Skill IDs ' + safe((bot?.skills ?? []).join(', ') || 'None') +
            '</li>';
        }).join('') + '</ol>' +
        '<p>Native Council Reviewers are independent read-only roles. ' +
        'Skill presence alone does not prove a running harness can invoke it.</p>';
    } else if (this.step === 4) {
      content = '<fieldset class="sa-mode-choices"><legend>Execution mode</legend>' +
        '<label class="sa-mode-option"><input type="radio" name="builderExecutionMode" value="approval"' +
          (this.executionMode === 'approval' ? ' checked' : '') +
          '><span><strong>With Approval</strong><small>Every completed WorkItem waits for an explicit human decision before dependents advance.</small></span></label>' +
        '<label class="sa-mode-option"><input type="radio" name="builderExecutionMode" value="full_auto"' +
          (this.executionMode === 'full_auto' ? ' checked' : '') +
          '><span><strong>Full Auto</strong><small>Plan and dispatch native WorkItems automatically. Independent native Council must verify each result to complete it.</small></span></label>' +
        '</fieldset>' +
        '<p>' + (this.executionMode === 'full_auto'
          ? 'Select at least two distinct native reviewer bots in Members. Without verified Council, work stops rather than being accepted.'
          : 'Human acceptance is required. No Council verdict can silently replace it.') + '</p>' +
        (this.executionMode === 'full_auto' && this.reviewers.length < 2
          ? '<button type="button" data-sa-builder-reviewers>Choose independent reviewers</button>'
          : '') +
        '<p>Both policies preserve each bot\'s model, skills and native conversation. Tool and GitHub effects still require specific native grants.</p>' +
        '<p>Saving a Team leaves it Off until explicitly enabled; no request or permission is created by configuration.</p>';
    } else {
      const problem = this.problem(bots, 4);
      content = '<dl><dt>Team</dt><dd>' + safe(this.name.trim()) + '</dd>' +
        '<dt>Coordinator</dt><dd>' + safe(name(this.coordinatorId)) + '</dd>' +
        '<dt>Project</dt><dd>' + safe(selectedBot(this.primaryWorkerId)?.projectId ?? 'Unknown') +
        '</dd><dt>Workers</dt><dd>' +
        [this.primaryWorkerId,...this.workers].map(name).map(safe).join(', ') +
        '</dd><dt>Reviewers</dt><dd>' +
        (this.reviewers.map(name).map(safe).join(', ') || 'None') + '</dd>' +
        '<dt>Native Council</dt><dd>' + (this.councilRequired ? 'Required' : 'Off') +
        '</dd><dt>Execution mode</dt><dd>' + (this.executionMode === 'full_auto' ? 'Full Auto' : 'With Approval') +
        '</dd><dt>Team state</dt><dd>Off until enabled; no task or tool grants</dd></dl>' +
        (problem ? '<p role="alert">' + safe(problem) + '</p>' :
          '<p>Save rechecks all native bot, project and versioned Team guards.</p>') +
        '<button class="primary-button" type="submit"' +
        (problem ? ' disabled' : '') + '>Create Team (disabled)</button>';
    }
    return '<section class="sa-team-builder" aria-label="Guided Team Builder">' +
      '<button type="button" class="sa-detail-back" data-sa-builder-cancel>← Cancel team setup</button>' +
      '<div class="sa-section-head"><div><h2>Create a Team</h2>' +
      '<p>Step ' + this.step + ' of 5 · configure once, reuse for separate Demands.</p></div></div>' +
      '<div class="sa-wizard-layout"><nav class="sa-step-nav" aria-label="Team setup progress">' +
      steps.map((label, index) =>
        '<button type="button" data-sa-builder-step="' + (index + 1) + '"' +
        (index >= this.step ? ' disabled' : '') +
        ' aria-current="' + (index + 1 === this.step ? 'step' : 'false') + '">' +
        '<span class="sa-step-number">' + (index + 1) + '</span><span>' +
        safe(label) + '</span></button>').join('') + '</nav>' +
      '<form class="sa-wizard-form" data-sa-form="guided-builder">' +
      '<header class="sa-wizard-head"><h3>' + safe(steps[this.step-1]) + '</h3>' +
      '<p>Choose existing PandaCode bots. This form cannot grant tools or run models.</p></header>' +
      '<section aria-label="Team setup step"><h4 class="sr-only" tabindex="-1">' +
      safe(steps[this.step-1]) + '</h4>' + content + '</section>' +
      (this.error ? '<p role="alert" class="sa-builder-error">' + safe(this.error) + '</p>' : '') +
      '<footer class="sa-wizard-footer">' +
      '<button type="button" data-sa-builder-prev' + (this.step === 1 ? ' disabled' : '') +
      '>Back</button>' +
      (this.step < 5 ? '<button type="button" class="primary-button" data-sa-builder-next>Next</button>' : '') +
      '</footer></form></div></section>';
  }
}
