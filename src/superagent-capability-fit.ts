/** Read-only native evidence view for matching declared bot capabilities to Team objectives. */
import { escape } from './dom';

export interface CapabilityCandidate {
  botId:string; name:string; suggestedRole:'worker'|'reviewer';
  alreadyAssigned:boolean; declaredMatchScore:number; matchedTerms:string[];
  evidenceSources:string[]; status:string;
  projectScope:{projectId:string;status:string};
  permissionMode:{value:string;status:string};
  availability:{status:string;paused:boolean};
  executionEligibility:string; sourceConfidence:string;
}
export interface CapabilityFitResult {
  orchestrationId:string; policyRevision:number; observedAt:number;
  candidates:CapabilityCandidate[]; excludedByPrivacyPreference:number;
  truncated:boolean; selectionApplied:boolean; authorizationGranted:boolean;
  disclaimer:string;
}

export function renderCapabilityFit(
  result:CapabilityFitResult|null, objective:string, workers:string[],
  reviewers:string[], enabled:boolean,
):string {
  const safe=(value:unknown)=>escape(String(value??''));
  const items=result?.candidates.map(candidate=>{
    const assigned=workers.includes(candidate.botId)||reviewers.includes(candidate.botId);
    const allowed=!enabled && !assigned && candidate.status!=='unavailable';
    const disabled=allowed?'':' disabled';
    const explanation=candidate.matchedTerms.length
      ? 'Matches: '+candidate.matchedTerms.map(safe).join(', ')
      : 'No declared term match; capabilities remain unknown';
    return '<li><strong>'+safe(candidate.name)+'</strong>' +
      ' <small>Declared fit '+candidate.declaredMatchScore+' · '+safe(candidate.status)+
      ' · suggested '+safe(candidate.suggestedRole)+'</small><p>'+explanation+'</p>'+
      '<p><small>Evidence: '+safe(candidate.evidenceSources.join(', ')||'none')+
      ' · Project '+safe(candidate.projectScope.projectId)+
      ' · Mode '+safe(candidate.permissionMode.value)+
      ' · Execution eligibility not verified</small></p>'+
      (assigned?'<span>Already staged or assigned</span>':
      '<button type="button" data-sa-add-candidate="'+safe(candidate.botId)+
      '" data-sa-candidate-role="worker"'+disabled+'>Stage as Worker</button> '+
      '<button type="button" data-sa-add-candidate="'+safe(candidate.botId)+
      '" data-sa-candidate-role="reviewer"'+disabled+'>Stage as Reviewer</button>')+'</li>';
  }).join('')??'';
  return '<section class="sa-pane" role="region" aria-label="Bot capability matching">' +
    '<h4>Discover candidate bot capabilities</h4>' +
    '<p>Compare self-declared roles, explicitly published summaries and assigned Skills. '+
    'Private instructions and histories are excluded. Matching never admits a Worker or Reviewer.</p>'+
    '<form data-sa-form="capability-scan"><label>Work objective'+
    '<input name="objective" maxlength="2000" required value="'+safe(objective)+
    '" placeholder="e.g. Rust testing and security review"></label>'+
    '<button type="submit">Compare current capabilities</button></form>'+
    (result?'<p role="status">'+result.candidates.length+' candidates · '+
      result.excludedByPrivacyPreference+' private profiles excluded · '+
      (result.truncated?'Showing first 30 · ':'')+'No authority granted</p>'+
      (items?'<ol class="sa-list">'+items+'</ol>':
      '<p>No discoverable candidates.</p>')+
      '<p>Buttons only stage suggestions. Save configured roles to validate the native Team policy.</p>'
      :'<p>Run a read-only comparison to see current evidence and restrictions.</p>')+
    '</section>';
}
