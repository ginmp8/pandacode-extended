/** Deterministic preview-only SuperAgent adapter. No real harness or external writes. */
type Bot = { id:string; name:string; paused:boolean; projectId:string };
type Config = { id:string; name:string; coordinatorBotId:string; workerBotId:string;
  workerBotIds:string[]; reviewerBotIds:string[]; teamVersion:number;
  projectId:string; enabled:boolean; policyRevision:number; councilRequired:boolean };
type Ticket = { id:string; orchestrationId:string; objective:string;
  requestKey:string; actor:string; state:string; revision:number; activeAssignmentId:string|null;
  evidence:string|null; createdAt:number; updatedAt:number; provenance?:string };
type WorkItem = { id:string; ticketId:string; objective:string; state:string;
  evidenceContract:string; ordinal:number; dependsOnIds?:string[]; readiness?:string;
  createdAt:number; updatedAt:number };
type TeamSnapshot = {
  snapshotVersion:number; provenance:string; orchestrationId:string; teamVersion:number;
  name:string; projectId:string; coordinatorBotId:string; workerBotIds:string[];
  reviewerBotIds:string[];
  policy:{policyRevision:number;autonomy:string;maxConcurrentTasks:number;
    maxAttemptsPerAssignment:number;councilEnabled:boolean;councilRequired:boolean};
  roles:Array<{role:string;ordinal:number;botId:string;effectiveConfig:null;effectiveConfigSha256:null}>;
};
type Activity = { id:number; kind:string; detail:string; at:number };
type Assignment = { id:string; botId:string; sessionId:string; attempt:number; state:string };
type ManifestPreset = { name:string; coordinatorBotId:string; workerBotId:string;
  workerBotIds:string[]; reviewerBotIds:string[]; projectId:string; councilRequired:boolean };
const activity = new Map<string,Activity[]>(), assignments = new Map<string,Assignment[]>();
const PREVIEW_OBSERVE_EVENT_KINDS = [
  'issues.opened','issues.edited','issue_comment.created','pull_request.opened',
  'pull_request_review.submitted','check_run.completed',
] as const;
const threadLinks = new Map<string,Array<{sourceId:string;ticketId:string;repository:string;subjectKey:string;
  actorFilter:string;eventKind:string;mode:'off'|'observe';createdAt:number}>>();
const configs = new Map<string,Config>(), tickets = new Map<string,Ticket[]>();
const teamSnapshots = new Map<string,TeamSnapshot>();
const workItems = new Map<string,WorkItem[]>();
const get = (id:string):Config => { const c=configs.get(id); if(!c)throw Error('Orchestration not found.'); return c; };
const previewParams = typeof location === 'undefined' ? new URLSearchParams() : new URLSearchParams(location.search);
let demandRunsSampleSeeded = false;

/** Native-shaped fixture data for the explicit `?superagent=sample` browser preview. */
function seedDemandRunsSample(bots:Bot[]):void {
  if(previewParams.get('superagent')!=='sample'||demandRunsSampleSeeded)return;
  const bot=(name:string):Bot=>{
    const found=bots.find(entry=>entry.name===name);
    if(!found)throw Error(`The SuperAgent sample needs the preview bot “${name}”.`);
    return found;
  };
  const scout=bot('Scout'),writer=bot('Writer'),ledger=bot('Ledger');
  const web:Config={id:'preview-team-web',name:'Web Delivery',coordinatorBotId:scout.id,
    workerBotId:writer.id,workerBotIds:[writer.id],reviewerBotIds:[],teamVersion:1,
    projectId:writer.projectId,enabled:true,policyRevision:2,councilRequired:false};
  const issues:Config={id:'preview-team-issues',name:'Issue Resolution',coordinatorBotId:ledger.id,
    workerBotId:scout.id,workerBotIds:[scout.id],reviewerBotIds:[],teamVersion:1,
    projectId:scout.projectId,enabled:true,policyRevision:3,councilRequired:false};
  configs.set(web.id,web);configs.set(issues.id,issues);
  const now=Date.now();
  const add=(config:Config,input:{id:string;objective:string;state:string;revision:number;
    evidence:string|null;age:number;assignment?:Assignment;activityKind:string;
    workItems?:WorkItem[]}):void=>{
    const createdAt=now-input.age,updatedAt=createdAt+60_000;
    const assignment=input.assignment?{...input.assignment}:undefined;
    const ticket:Ticket={id:input.id,orchestrationId:config.id,
      requestKey:`preview-request-${input.id}`,objective:input.objective,actor:'local-user',
      state:input.state,revision:input.revision,
      activeAssignmentId:assignment?.id??null,evidence:input.evidence,createdAt,updatedAt,
      provenance:'preview-fixture'};
    tickets.set(config.id,[...(tickets.get(config.id)||[]),ticket]);
    const roleIds=[config.coordinatorBotId,...config.workerBotIds,...config.reviewerBotIds];
    teamSnapshots.set(ticket.id,{snapshotVersion:1,provenance:'preview-fixture',
      orchestrationId:config.id,teamVersion:config.teamVersion,name:config.name,
      projectId:config.projectId,coordinatorBotId:config.coordinatorBotId,
      workerBotIds:[...config.workerBotIds],reviewerBotIds:[...config.reviewerBotIds],
      policy:{policyRevision:config.policyRevision,autonomy:'off',maxConcurrentTasks:1,
        maxAttemptsPerAssignment:2,councilEnabled:false,councilRequired:false},
      roles:roleIds.map((botId,index)=>({role:index===0?'coordinator':'worker',
        ordinal:index===0?0:index-1,botId,effectiveConfig:null,effectiveConfigSha256:null}))});
    assignments.set(ticket.id,assignment?[assignment]:[]);
    activity.set(ticket.id,[{id:1,kind:input.activityKind,
      detail:'Native-shaped preview fixture event; no Ticket transition was executed.',at:updatedAt}]);
    workItems.set(ticket.id,input.workItems?.map(item=>({...item,ticketId:ticket.id,
      createdAt,updatedAt}))||[]);
  };
  const assignment=(id:string,botId:string,state:string,attempt=1):Assignment=>({
    id:`preview-assignment-${id}`,botId,sessionId:`preview-session-${id}`,
    attempt,state,
  });
  add(web,{id:'preview-ticket-web-104',objective:'Add keyboard navigation to the search panel',
    state:'running',revision:3,evidence:null,age:20*60_000,activityKind:'worker_started',
    assignment:assignment('web-104',writer.id,'running'),workItems:[
      {id:'preview-work-web-104-1',ticketId:'',objective:'Implement keyboard navigation',
        evidenceContract:'Keyboard users can move through search results.',state:'running',ordinal:0,
        dependsOnIds:[],readiness:'active',createdAt:0,updatedAt:0},
      {id:'preview-work-web-104-2',ticketId:'',objective:'Verify focus order',
        evidenceContract:'Focus order remains visible and predictable.',state:'queued',ordinal:1,
        dependsOnIds:['preview-work-web-104-1'],readiness:'dependency_blocked',createdAt:0,updatedAt:0},
    ]});
  add(web,{id:'preview-ticket-web-103',objective:'Document the search empty state',
    state:'waiting_approval',revision:2,evidence:'Preview worker evidence: documented the empty search state.',
    age:90*60_000,activityKind:'worker_evidence_ready',
    assignment:assignment('web-103',writer.id,'finished')});
  add(issues,{id:'preview-ticket-issue-203',objective:'Investigate the flaky sidebar assertion',
    state:'blocked',revision:4,evidence:null,age:3*60*60_000,activityKind:'worker_blocked',
    assignment:assignment('issue-203',scout.id,'blocked',2),workItems:[
      {id:'preview-work-issue-203-1',ticketId:'',objective:'Reproduce the flaky assertion',
        evidenceContract:'Record the failing condition and reproduction.',state:'blocked',ordinal:0,
        dependsOnIds:[],readiness:'not_ready',createdAt:0,updatedAt:0},
    ]});
  add(issues,{id:'preview-ticket-issue-202',objective:'Fix issue 73: preserve filters after refresh',
    state:'queued',revision:1,evidence:null,age:4*60*60_000,activityKind:'ticket_queued'});
  demandRunsSampleSeeded=true;
}

/** Build a retry transition from native-shaped state without rewriting terminal records. */
export function preparePreviewRetryState(input:{ticketId:string;objective:string;workerBotId:string;
  maxAttemptsPerAssignment:number;items:WorkItem[];assignments:Assignment[];
  latestActivity?:Activity;now?:number}):{assignment:Assignment;workItems:WorkItem[]} {
  const {ticketId,objective,workerBotId,maxAttemptsPerAssignment,items,assignments}=input;
  const item=items[0],workerHistory=assignments.filter(entry=>entry.botId===workerBotId);
  const latestWorkerAssignment=workerHistory[workerHistory.length-1];
  const correctionRetry=items.length===1&&item?.ticketId===ticketId&&
    item.state==='completed'&&item.objective===objective&&
    latestWorkerAssignment?.state==='finished'&&input.latestActivity?.kind==='human_rejected';
  const failedWorkItemRetry=items.length===1&&item?.ticketId===ticketId&&
    ['blocked','failed'].includes(item.state);
  if(!correctionRetry&&!failedWorkItemRetry)
    throw Error('Preview retry requires one recoverable WorkItem or an explicitly rejected completed WorkItem.');
  if(workerHistory.length>=maxAttemptsPerAssignment)
    throw Error('Manual worker retry limit reached; create a new authorized ticket.');
  const attempt=Math.max(0,...assignments.map(entry=>entry.attempt))+1;
  const now=input.now??Date.now();
  const assignment:Assignment={id:`preview-assignment-${ticketId}-attempt-${attempt}`,
    botId:workerBotId,sessionId:`preview-session-${ticketId}-attempt-${attempt}`,attempt,state:'running'};
  const workItems=correctionRetry
    ? [...items,{id:`preview-work-item-${ticketId}-correction-${attempt}`,ticketId,
        objective,evidenceContract:item?.evidenceContract ||
          `Preview-only informational description: record the observable result for "${objective}".`,
        state:'running',ordinal:Math.max(-1,...items.map(workItem=>workItem.ordinal))+1,
        createdAt:now,updatedAt:now}]
    : items.map(workItem=>({...workItem,state:'running',updatedAt:now}));
  return {assignment,workItems};
}

export async function invokeSuperagent<T>(
  command:string,args:Record<string,any>,listBots:()=>Promise<Bot[]>,
):Promise<T> {
  // Pure listing must not race the preview's first bots_list seed. Only
  // commands that actually resolve portable bot identities may load bots.
  const needsBots=['superagent_create','superagent_update',
    'superagent_manifest_export','superagent_manifest_preview','superagent_manifest_import'].includes(command)||
    (command==='superagent_list'&&previewParams.get('superagent')==='sample');
  const bots=needsBots?await listBots():[];
  if(command==='superagent_list'&&previewParams.get('superagent')==='sample')seedDemandRunsSample(bots);
  const find=(id:string)=>{ const b=bots.find(b=>b.id===id); if(!b)throw Error('Bot no longer exists.'); return b; };
  const configName=(name:string)=>[...configs.values()].find(c=>c.name.toLowerCase()===name.toLowerCase());
  const resolveRoles=(input:any)=>{
    if(!input||typeof input!=='object')throw Error('Invalid Team input.');
    const workerBotIds=Array.isArray(input.workerBotIds)?input.workerBotIds:[input.workerBotId];
    const reviewerBotIds=Array.isArray(input.reviewerBotIds)?input.reviewerBotIds:[];
    if(workerBotIds.length<1||workerBotIds.length>16||reviewerBotIds.length>8||
      workerBotIds.some((id:unknown)=>typeof id!=='string'||!id)||
      reviewerBotIds.some((id:unknown)=>typeof id!=='string'||!id)||
      workerBotIds[0]!==input.workerBotId)
      throw Error('A Team needs one coordinator, 1–16 workers and at most 8 reviewers, with workerBotId matching the first worker.');
    const coordinator=find(input.coordinatorBotId);
    const workers=(workerBotIds as string[]).map(find);
    const reviewers=(reviewerBotIds as string[]).map(find);
    const ids=[coordinator.id,...workers.map(bot=>bot.id),...reviewers.map(bot=>bot.id)];
    if(new Set(ids).size!==ids.length)throw Error('A bot can have only one role in a Team.');
    if(ids.some(id=>find(id).projectId!==workers[0].projectId))
      throw Error('All Team roles must belong to the same project.');
    return {coordinator,workers,reviewers,workerBotIds:[...workerBotIds] as string[],
      reviewerBotIds:[...reviewerBotIds] as string[]};
  };
  // Native policy is off or required. Create without the field means Off;
  // update without it preserves the current Team policy, like the native command.
  const councilRequirement=(input:any,fallback:boolean):boolean=>{
    if(input?.councilRequired===undefined)return fallback;
    if(typeof input.councilRequired!=='boolean')throw Error('Invalid Team input.');
    return input.councilRequired;
  };
  const validCouncilReviewers=(reviewerBotIds:string[],councilRequired:boolean):void=>{
    if(councilRequired&&(reviewerBotIds.length<2||reviewerBotIds.length>8))
      throw Error('Council-required Teams need between two and eight selected native reviewers.');
  };
  // Portable names resolve to exactly one live Bot, like the native manifest.
  const portableBot=(name:unknown):Bot=>{
    if(typeof name!=='string'||!name.trim())throw Error('Invalid portable bot reference.');
    const matches=bots.filter(bot=>bot.name.toLowerCase()===name.toLowerCase());
    if(matches.length!==1)throw Error('Invalid portable bot reference.');
    return matches[0];
  };
  const manifestV1Preset=(preset:any):ManifestPreset=>{
    if(!preset||typeof preset!=='object')throw Error('Invalid manifest.');
    if(preset?.autonomy!=='off'||preset?.council?.enabled||preset?.maxConcurrentTasks!==1)
      throw Error('Unsupported policy.');
    const coordinator=portableBot(preset.coordinatorBot),worker=portableBot(preset.workerBot);
    if(coordinator.id===worker.id)throw Error('Invalid portable bot reference.');
    return {name:preset.name,coordinatorBotId:coordinator.id,workerBotId:worker.id,
      workerBotIds:[worker.id],reviewerBotIds:[],projectId:worker.projectId,councilRequired:false};
  };
  const optionalBoolean=(value:unknown):boolean=>{
    if(value===undefined)return false;
    if(typeof value!=='boolean')throw Error('Invalid manifest policy.');
    return value;
  };
  const manifestV2Preset=(preset:any):ManifestPreset=>{
    if(!preset||typeof preset!=='object')throw Error('Invalid manifest.');
    const policy=preset.policy;
    if(!policy||typeof policy!=='object'||policy.autonomy!=='off'||
      policy.maxConcurrentTasks!==1||policy.maxAttemptsPerAssignment!==2)
      throw Error('Unsupported policy.');
    // Native normalizes either wire flag to the required Council gate.
    const councilRequired=optionalBoolean(policy.councilRequired)||
      optionalBoolean(policy.councilEnabled);
    const workerNames=preset.workers,reviewerNames=preset.reviewers??[];
    if(!Array.isArray(workerNames)||workerNames.length<1||workerNames.length>16)
      throw Error('A Team needs 1–16 workers.');
    if(!Array.isArray(reviewerNames)||reviewerNames.length>8)
      throw Error('A Team needs at most 8 reviewers.');
    const coordinator=portableBot(preset.coordinatorBot);
    const workers=(workerNames as unknown[]).map(portableBot);
    const reviewers=(reviewerNames as unknown[]).map(portableBot);
    const ids=[coordinator.id,...workers.map(bot=>bot.id),...reviewers.map(bot=>bot.id)];
    if(new Set(ids).size!==ids.length)throw Error('A bot can have only one role in a Team.');
    const projectId=workers[0].projectId;
    if(workers.some(bot=>bot.projectId!==projectId))
      throw Error('All Team workers must share the same project.');
    const distinctReviewers=new Set(reviewers.map(bot=>bot.id)).size;
    if(councilRequired&&(distinctReviewers<2||distinctReviewers>8))
      throw Error(`Council-required Team '${String(preset.name)}' needs between two and eight distinct reviewers.`);
    return {name:preset.name,coordinatorBotId:coordinator.id,workerBotId:workers[0].id,
      workerBotIds:workers.map(bot=>bot.id),reviewerBotIds:reviewers.map(bot=>bot.id),
      projectId,councilRequired};
  };
  const makeTeamSnapshot=(config:Config):TeamSnapshot=>({
    snapshotVersion:1,provenance:'preview-fixture',orchestrationId:config.id,teamVersion:config.teamVersion,
    name:config.name,projectId:config.projectId,coordinatorBotId:config.coordinatorBotId,
    workerBotIds:[...config.workerBotIds],reviewerBotIds:[...config.reviewerBotIds],
    policy:{policyRevision:config.policyRevision,autonomy:'off',maxConcurrentTasks:1,
      maxAttemptsPerAssignment:2,councilEnabled:config.councilRequired,
      councilRequired:config.councilRequired},
    roles:[
      {role:'coordinator',ordinal:0,botId:config.coordinatorBotId,effectiveConfig:null,effectiveConfigSha256:null},
      ...config.workerBotIds.map((botId,ordinal)=>({role:'worker',ordinal,botId,
        effectiveConfig:null as null,effectiveConfigSha256:null as null})),
      ...config.reviewerBotIds.map((botId,ordinal)=>({role:'reviewer',ordinal,botId,
        effectiveConfig:null as null,effectiveConfigSha256:null as null})),
    ],
  });
  let value:unknown;
  switch(command){
    case 'superagent_list': value=[...configs.values()].map(config=>{
      const teamTickets=tickets.get(config.id)||[];
      return {...config,
        ticketCount:teamTickets.length,
        runningCount:teamTickets.filter(ticket=>ticket.state==='running').length,
        blockedCount:teamTickets.filter(ticket=>ticket.state==='blocked').length,
        lastActivityAt:teamTickets.length?Math.max(...teamTickets.map(ticket=>ticket.updatedAt)):undefined,
      };
    });break;
    case 'superagent_tickets':{
      const limit=args.limit===undefined?100:Number(args.limit);
      if(!Number.isInteger(limit)||limit<1||limit>200)throw Error('Ticket page size must be between 1 and 200.');
      value=[...(tickets.get(args.orchestrationId)||[])]
        .sort((left,right)=>right.createdAt-left.createdAt||right.id.localeCompare(left.id)).slice(0,limit);
      break;
    }
    case 'superagent_work_items':{
      const ticketId=typeof args.ticketId==='string'?args.ticketId.trim():'';
      if(!ticketId)throw Error('A Ticket id is required.');
      const ticket=[...tickets.values()].flat().find(item=>item.id===ticketId);
      if(!ticket)throw Error('Ticket not found.');
      value=[...(workItems.get(ticketId)||[])].sort((left,right)=>left.ordinal-right.ordinal||left.id.localeCompare(right.id));
      break;
    }
    case 'superagent_snapshot':
    case 'superagent_mandate':
    case 'superagent_assignments':
    case 'superagent_activity':{
      const t=[...tickets.values()].flat().find(item=>item.id===args.ticketId);
      if(!t)throw Error('Ticket not found.');
      const cfg=get(t.orchestrationId);
      const mandate={ticketId:t.id,actor:'local-user',authorizationId:'preview-only-'+t.id,
        policyRevision:cfg.policyRevision,projectId:cfg.projectId,
        expiresAt:Date.now()+4*60*60*1000,revoked:t.state==='stopped',
        allowedTools:[],allowedSideEffects:false,createdAt:Date.now()};
      if(command==='superagent_assignments')value=assignments.get(t.id)||[];
      else if(command==='superagent_activity')value=activity.get(t.id)||[];
      else if(command==='superagent_mandate')value=mandate;
      else value={ticket:t,teamSnapshot:teamSnapshots.get(t.id)||null,mandate,
        assignments:assignments.get(t.id)||[],activity:activity.get(t.id)||[]};
      break;
    }
    case 'superagent_source_thread_links':
      value=threadLinks.get(args.ticketId)||[];break;
    case 'superagent_source_fetch_thread':
      value={previewOnly:true,message:`Preview only. Manual Observe supports ${PREVIEW_OBSERVE_EVENT_KINDS.join(', ')}. No GitHub request was sent and no event or decision receipt was stored.`};break;
    // These are preview-only references. Real native code verifies source and
    // Ticket orchestration ownership and rejects conflicting thread reuse.
    case 'superagent_source_link_thread':
      throw Error('Source linking requires a native trusted event source.');
    case 'superagent_create':{
      const input=args.input,roles=resolveRoles(input),b=roles.workers[0];
      const councilRequired=councilRequirement(input,false);
      validCouncilReviewers(roles.reviewerBotIds,councilRequired);
      if(!input.name?.trim()||configName(input.name))throw Error('Invalid or duplicate orchestration.');
      const c:Config={id:crypto.randomUUID(),name:input.name,coordinatorBotId:roles.coordinator.id,
        workerBotId:b.id,workerBotIds:roles.workerBotIds,reviewerBotIds:roles.reviewerBotIds,
        teamVersion:1,projectId:b.projectId,enabled:false,policyRevision:1,councilRequired};
      configs.set(c.id,c);value=c;break;
    }
    case 'superagent_update':{
      const c=get(args.orchestrationId),input=args.input;
      if(c.enabled||c.policyRevision!==args.expectedRevision||(tickets.get(c.id)?.length||0)>0)
        throw Error('Preset cannot change after use or while enabled.');
      const councilRequired=councilRequirement(input,c.councilRequired);
      const roles=resolveRoles(input),b=roles.workers[0];
      validCouncilReviewers(roles.reviewerBotIds,councilRequired);
      if(!input.name?.trim()||configName(input.name)?.id!==undefined&&configName(input.name)?.id!==c.id)
        throw Error('Invalid or duplicate preset.');
      c.name=input.name;c.coordinatorBotId=roles.coordinator.id;c.workerBotId=b.id;
      c.workerBotIds=roles.workerBotIds;c.reviewerBotIds=roles.reviewerBotIds;
      c.projectId=b.projectId;c.teamVersion++;c.policyRevision++;c.councilRequired=councilRequired;
      value=c;break;
    }
    case 'superagent_set_enabled':{
      const c=get(args.orchestrationId);if(c.policyRevision!==args.expectedRevision)throw Error('Stale policy.');
      if(c.enabled!==args.enabled){c.enabled=args.enabled;c.policyRevision++;}
      value=c;break;
    }
    case 'superagent_delete':{
      const c=get(args.orchestrationId);if(c.enabled||c.policyRevision!==args.expectedRevision||
        (tickets.get(c.id)?.length||0)>0)throw Error('Cannot delete used or enabled preset.');
      configs.delete(c.id);value=null;break;
    }
    case 'superagent_submit':
    case 'superagent_submit_coordinated':{
      const c=get(args.orchestrationId);if(!c.enabled)throw Error('Orchestration disabled.');
      const coordinated=command==='superagent_submit_coordinated';
      if(coordinated)
        throw Error('Preview cannot run the Coordinator. Coordinated Ticket submission is unavailable.');
      const now=Date.now(),ticketId=crypto.randomUUID(),assignmentId='preview-assignment-'+ticketId;
      const t:Ticket={id:ticketId,orchestrationId:c.id,requestKey:String(args.requestKey??''),
        objective:args.objective,actor:'local-user',state:'running',revision:0,
        activeAssignmentId:assignmentId,evidence:null,createdAt:now,updatedAt:now};
      teamSnapshots.set(t.id,makeTeamSnapshot(c));
      tickets.set(c.id,[...(tickets.get(c.id)||[]),t]);
      assignments.set(t.id,[{id:assignmentId,botId:c.workerBotId,
        sessionId:'preview-session-'+t.id,attempt:1,state:'running'}]);
      workItems.set(t.id,[{id:'preview-work-item-'+t.id,ticketId:t.id,
        objective:t.objective,
        evidenceContract:`Preview-only informational description: record the observable result for "${t.objective}".`,
        state:'running',ordinal:0,createdAt:now,updatedAt:now}]);
      activity.set(t.id,[{id:1,kind:'admitted',detail:'Manual read-only preview assignment',at:now}]);
      value=t;break;
    }
    case 'superagent_stop':
    case 'superagent_retry':
    case 'superagent_dispatch_worker':
    case 'superagent_accept':
    case 'superagent_reject':{
      const t=[...tickets.values()].flat().find(t=>t.id===args.ticketId);
      if(!t)throw Error('Ticket not found.');
      if(command==='superagent_stop') {
        if(['completed','stopped','failed'].includes(t.state))throw Error('Ticket already finished.');
        t.state='stopped';
      }else if(command==='superagent_dispatch_worker') {
        const config=get(t.orchestrationId);
        if(!config.enabled)throw Error('Orchestration disabled.');
        if(t.state!=='queued')throw Error('No coordinator plan.');
        const snapshot=teamSnapshots.get(t.id),history=assignments.get(t.id)||[];
        const coordinatorAssignment=history[0];
        const workerBotId=snapshot?.workerBotIds[0];
        if(!snapshot||history.length!==1||!coordinatorAssignment||
          coordinatorAssignment.botId!==snapshot.coordinatorBotId||
          coordinatorAssignment.attempt!==1||coordinatorAssignment.state!=='finished'||!workerBotId)
          throw Error('Preview Coordinator assignment is unavailable.');
        if((workItems.get(t.id)||[]).length!==0)
          throw Error('Preview worker admission already has a WorkItem.');
        // The preview has no structured Coordinator plan or evidence. Refuse
        // before touching the Ticket, Assignment history or WorkItems.
        throw Error('Preview cannot dispatch a Worker without recorded Coordinator evidence and a plan.');
      }else if(command==='superagent_retry') {
        if(t.state!=='blocked')throw Error('Only a blocked SuperAgent ticket can be retried manually.');
        const items=workItems.get(t.id)||[];
        const config=get(t.orchestrationId),snapshot=teamSnapshots.get(t.id);
        const workerBotId=snapshot?.workerBotIds[0];
        if(!config.enabled)throw Error('Orchestration disabled.');
        if(!snapshot||!workerBotId)throw Error('Saved preview worker is unavailable.');
        const history=assignments.get(t.id)||[];
        const transition=preparePreviewRetryState({ticketId:t.id,objective:t.objective,workerBotId,
          maxAttemptsPerAssignment:snapshot.policy.maxAttemptsPerAssignment,items,assignments:history,
          latestActivity:(activity.get(t.id)||[])[0]});
        history.push(transition.assignment);
        assignments.set(t.id,history);
        workItems.set(t.id,transition.workItems);
        t.state='running';
        t.evidence=null;
      }else {
        if(t.state!=='waiting_approval'||t.revision!==args.expectedRevision)throw Error('Stale worker evidence.');
        t.state=command==='superagent_accept'?'completed':'blocked';
      }
      t.revision++;t.updatedAt=Date.now();
      if(command==='superagent_stop'){
        for(const entry of assignments.get(t.id)||[])if(entry.state==='running')entry.state='stopped';
        workItems.set(t.id,(workItems.get(t.id)||[]).map(item=>item.state==='running'
          ? {...item,state:'blocked',updatedAt:Date.now()} : item));
      }
      const events=activity.get(t.id)||[];
      events.unshift({id:events.length+1,kind:command==='superagent_reject'
        ? 'human_rejected':command.slice('superagent_'.length),
        detail:'Preview-only state transition',at:Date.now()});
      activity.set(t.id,events);
      value=t;break;
    }
    case 'superagent_dispatch_worker_with_write_grant':{
      const t=[...tickets.values()].flat().find(t=>t.id===args.ticketId);
      if(!t)throw Error('Ticket not found.');
      if(t.revision!==args.expectedRevision)throw Error('Ticket revision changed before preview.');
      const paths=args.allowedPaths;
      if(!Array.isArray(paths)||paths.length===0||paths.length>16||paths.some((path:unknown)=>typeof path!=='string'))
        throw Error('Authorize between 1 and 16 exact file paths.');
      const unique=new Set<string>();
      for(const path of paths as string[]){
        const invalidSegment=path.split('/').some(part=>part===''||part==='.'||part==='..'||
          part.toLowerCase()==='.git'||[...part].some(character=>{
            const code=character.codePointAt(0)??0;return code<=0x1f||(code>=0x7f&&code<=0x9f);
          }));
        if(new TextEncoder().encode(path).length>4096||path.includes('\\')||path.includes('\0')||
           path.includes(':')||invalidSegment)throw Error('Invalid exact project-relative file path.');
        if(unique.has(path))throw Error('A write scope cannot repeat a file path.');
        unique.add(path);
      }
      value={previewOnly:true,ticket:t,allowedPaths:[...(paths as string[])],
        message:'Preview only. No native authorization or worker dispatch occurred.'};
      break;
    }
    case 'superagent_verify_draft_pull_request_target':{
      value='e'.repeat(40);
      break;
    }
    case 'superagent_issue_comment':{
      value={previewOnly:true,message:'Preview only. No GitHub comment was sent.'};
      break;
    }
    case 'superagent_create_draft_pull_request':{
      value={previewOnly:true,message:'Preview only. No GitHub pull request was created.'};
      break;
    }
    case 'superagent_manifest_export':{
      const presets=[...configs.values()];
      // Native v2 is required to carry a required Council gate truthfully.
      // All-off stores keep the original v1 shape, which never enables Council.
      value=presets.some(c=>c.councilRequired)
        ? {schemaVersion:2,orchestrations:presets.map(c=>({
            name:c.name,coordinatorBot:find(c.coordinatorBotId).name,
            workers:c.workerBotIds.map(id=>find(id).name),
            reviewers:c.reviewerBotIds.map(id=>find(id).name),
            policy:{autonomy:'off',maxConcurrentTasks:1,maxAttemptsPerAssignment:2,
              councilEnabled:c.councilRequired,councilRequired:c.councilRequired},
          }))}
        : {schemaVersion:1,orchestrations:presets.map(c=>({
            name:c.name,coordinatorBot:find(c.coordinatorBotId).name,
            workerBot:find(c.workerBotId).name,enabled:c.enabled,
            autonomy:'off',maxConcurrentTasks:1,maxAttemptsPerAssignment:2,
            council:{enabled:false},
          }))};
      break;
    }
    case 'superagent_manifest_preview':
    case 'superagent_manifest_import':{
      const doc=args.document;
      if(!doc||typeof doc!=='object'||!Array.isArray(doc.orchestrations)||
        (doc.schemaVersion!==1&&doc.schemaVersion!==2))throw Error('Invalid manifest.');
      const presets:ManifestPreset[]=doc.orchestrations.map((preset:any)=>
        doc.schemaVersion===1?manifestV1Preset(preset):manifestV2Preset(preset));
      const pending:ManifestPreset[]=[],unchanged:string[]=[],names=new Set<string>();
      for(const preset of presets){
        if(typeof preset.name!=='string'||!preset.name.trim()||names.has(preset.name.toLowerCase()))
          throw Error('Invalid or duplicate orchestration name in manifest.');
        names.add(preset.name.toLowerCase());
        const found=configName(preset.name);
        if(found){
          const sameRoles=found.coordinatorBotId===preset.coordinatorBotId&&
            found.workerBotIds.length===preset.workerBotIds.length&&
            found.workerBotIds.every((id,index)=>id===preset.workerBotIds[index])&&
            found.reviewerBotIds.length===preset.reviewerBotIds.length&&
            found.reviewerBotIds.every((id,index)=>id===preset.reviewerBotIds[index]);
          if(!sameRoles||found.councilRequired!==preset.councilRequired)
            throw Error('Conflicting preset.');
          unchanged.push(preset.name);
        }else pending.push(preset);
      }
      if(command==='superagent_manifest_import'){
        if(args.confirmed!==true)throw Error('Confirmation required.');
        for(const preset of pending){
          const c:Config={id:crypto.randomUUID(),name:preset.name,
            coordinatorBotId:preset.coordinatorBotId,workerBotId:preset.workerBotId,
            workerBotIds:[...preset.workerBotIds],reviewerBotIds:[...preset.reviewerBotIds],
            teamVersion:1,projectId:preset.projectId,enabled:false,policyRevision:1,
            councilRequired:preset.councilRequired};configs.set(c.id,c);
        }
        value={created:pending.map(preset=>preset.name),unchanged};
      }else value={wouldCreate:pending.map(preset=>preset.name),unchanged,
        importedPresetsDisabled:true};
      break;
    }
    default: throw Error('Unknown preview SuperAgent command: '+command);
  }
  return structuredClone(value) as T;
}
