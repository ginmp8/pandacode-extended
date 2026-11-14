/** Deterministic preview-only SuperAgent adapter. No real harness or external writes. */
type Bot = { id:string; name:string; paused:boolean; projectId:string };
type Config = { id:string; name:string; coordinatorBotId:string; workerBotId:string;
  workerBotIds:string[]; reviewerBotIds:string[]; teamVersion:number;
  projectId:string; enabled:boolean; policyRevision:number; councilRequired?:boolean };
type StrategyPolicy = {
  priority:string; attemptTimeoutMs:number; tokenCeiling?:number;
  estimatedCostCeilingMinor?:number; subagentDelegationEnabled:boolean;
};
type Strategy = {settings:StrategyPolicy;maxConcurrentTasks:number;maxAttemptsPerAssignment:number};
const defaultStrategy = ():Strategy => ({
  settings:{priority:'balanced',attemptTimeoutMs:1_800_000,
    subagentDelegationEnabled:false},
  maxConcurrentTasks:1,maxAttemptsPerAssignment:2,
});
type Ticket = { id:string; orchestrationId:string; objective:string;
  state:string; revision:number; evidence:null };
type TeamSnapshot = {
  snapshotVersion:number; provenance:string; orchestrationId:string; teamVersion:number;
  name:string; projectId:string; coordinatorBotId:string; workerBotIds:string[];
  reviewerBotIds:string[];
  policy:{policyRevision:number;autonomy:string;maxConcurrentTasks:number;
    maxAttemptsPerAssignment:number;councilEnabled:boolean;strategy:StrategyPolicy};
  roles:Array<{role:string;ordinal:number;botId:string;effectiveConfig:null;effectiveConfigSha256:null}>;
};
type Activity = { id:number; kind:string; detail:string; at:number };
type Assignment = { id:string; botId:string; sessionId:string; attempt:number; state:string };
const activity = new Map<string,Activity[]>(), assignments = new Map<string,Assignment[]>();
const PREVIEW_OBSERVE_EVENT_KINDS = [
  'issues.opened','issues.edited','issue_comment.created','pull_request.opened',
  'pull_request_review.submitted','check_run.completed',
] as const;
const threadLinks = new Map<string,Array<{sourceId:string;ticketId:string;repository:string;subjectKey:string;
  actorFilter:string;eventKind:string;mode:'off'|'observe';createdAt:number}>>();
const configs = new Map<string,Config>(), tickets = new Map<string,Ticket[]>();
const teamSnapshots = new Map<string,TeamSnapshot>();
const teamStrategies = new Map<string,Strategy>();
const strategyFor = (id:string):Strategy => teamStrategies.get(id) ?? defaultStrategy();
const get = (id:string):Config => { const c=configs.get(id); if(!c)throw Error('Orchestration not found.'); return c; };
export async function invokeSuperagent<T>(
  command:string,args:Record<string,any>,listBots:()=>Promise<Bot[]>,
):Promise<T> {
  // Pure listing must not race the preview's first bots_list seed. Only
  // commands that actually resolve portable bot identities may load bots.
  const needsBots=['superagent_create','superagent_update',
    'superagent_manifest_export','superagent_manifest_preview','superagent_manifest_import'].includes(command);
  const bots=needsBots?await listBots():[];
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
    // Match native Team admission: each existing bot retains its own project;
    // only executor Workers must share the scoped checkout. Independent
    // coordinator and read-only reviewer bots may own different projects.
    if(workers.some(bot=>bot.projectId!==workers[0].projectId))
      throw Error('Executor Workers must share one project scope.');
    if(input.councilRequired && reviewerBotIds.length < 2)
      throw Error('Council-required Teams need two independent reviewers.');
    return {coordinator,workers,reviewers,workerBotIds:[...workerBotIds] as string[],
      reviewerBotIds:[...reviewerBotIds] as string[]};
  };
  const makeTeamSnapshot=(config:Config):TeamSnapshot=>({
    snapshotVersion:1,provenance:'native',orchestrationId:config.id,teamVersion:config.teamVersion,
    name:config.name,projectId:config.projectId,coordinatorBotId:config.coordinatorBotId,
    workerBotIds:[...config.workerBotIds],reviewerBotIds:[...config.reviewerBotIds],
    policy:{policyRevision:config.policyRevision,autonomy:'off',
      maxConcurrentTasks:strategyFor(config.id).maxConcurrentTasks,
      maxAttemptsPerAssignment:strategyFor(config.id).maxAttemptsPerAssignment,
      councilEnabled:config.councilRequired===true,strategy:strategyFor(config.id).settings},
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
    case 'superagent_list': value=[...configs.values()].map(config=>({
      ...config,
      ticketCount:(tickets.get(config.id)||[]).length,
      runningCount:(tickets.get(config.id)||[]).filter(ticket=>ticket.state==='running').length,
      blockedCount:(tickets.get(config.id)||[]).filter(ticket=>ticket.state==='blocked').length,
      lastActivityAt:Date.now(),
    }));break;
    case 'superagent_roster':{
      const c=get(args.orchestrationId);
      value={orchestrationId:c.id,policyRevision:c.policyRevision,
        coordinatorBotId:c.coordinatorBotId,workerBotIds:[...c.workerBotIds],
        reviewerBotIds:[...c.reviewerBotIds],councilRequired:!!c.councilRequired,
        executionReady:c.workerBotIds.length===1 && c.reviewerBotIds.length===0};
      break;
    }
    case 'superagent_strategy_get':
    case 'superagent_strategy_update': {
      const config = get(args.orchestrationId);
      let saved = strategyFor(config.id);
      if(command==='superagent_strategy_update'){
        const request=args.input;
        if(config.enabled || config.policyRevision!==args.expectedRevision ||
           (tickets.get(config.id)||[]).some(ticket=>!['completed','failed','stopped','interrupted','inconclusive'].includes(ticket.state)))
          throw Error('Settle Tickets and disable this Team before changing strategy.');
        const validPriority=['balanced','cost','latency','throughput','quality'].includes(request?.priority);
        if(!validPriority || !Number.isInteger(request.maxConcurrentTasks) ||
           request.maxConcurrentTasks<1 || request.maxConcurrentTasks>8 ||
           !Number.isInteger(request.maxAttemptsPerAssignment) ||
           request.maxAttemptsPerAssignment<1 || request.maxAttemptsPerAssignment>8 ||
           !Number.isInteger(request.attemptTimeoutMs) ||
           request.attemptTimeoutMs<60_000 || request.attemptTimeoutMs>1_800_000 ||
           [request.tokenCeiling,request.estimatedCostCeilingMinor]
            .some(value=>value!==undefined && (!Number.isSafeInteger(value)||value<=0)))
          throw Error('Invalid bounded Team strategy.');
        const next:Strategy={
          settings:{priority:request.priority,attemptTimeoutMs:request.attemptTimeoutMs,
            tokenCeiling:request.tokenCeiling,
            estimatedCostCeilingMinor:request.estimatedCostCeilingMinor,
            subagentDelegationEnabled:request.subagentDelegationEnabled===true},
          maxConcurrentTasks:request.maxConcurrentTasks,
          maxAttemptsPerAssignment:request.maxAttemptsPerAssignment,
        };
        if(JSON.stringify(saved)!==JSON.stringify(next)){
          saved=next;teamStrategies.set(config.id,saved);
          config.policyRevision++;config.teamVersion++;
        }
      }
      const enableBlockers = [
        ...(saved.maxConcurrentTasks!==1
          ? ['Parallel writes over one leased checkout are not supported; the current native scheduler serializes conflicting workers.'] : []),
        ...(saved.settings.tokenCeiling!==undefined
          ? ['A native provider token meter is not available for a hard token ceiling.'] : []),
        ...(saved.settings.estimatedCostCeilingMinor!==undefined
          ? ['A verified price/usage meter is not available for a hard monetary ceiling.'] : []),
        ...(saved.settings.subagentDelegationEnabled
          ? ['The Team strategy alone cannot authorize descendant execution; a separate exact native delegation policy is required.'] : []),
      ];
      value={orchestrationId:config.id,policyRevision:config.policyRevision,
        teamVersion:config.teamVersion,settings:{...saved.settings},
        maxConcurrentTasks:saved.maxConcurrentTasks,
        maxAttemptsPerAssignment:saved.maxAttemptsPerAssignment,
        executableParallelWorkers:1,tokenMeteringAvailable:false,
        pricedCostMeteringAvailable:false,nestedChildAdmissionAvailable:true,enableBlockers};
      break;
    }
    case 'superagent_tickets':value=tickets.get(args.orchestrationId)||[];break;
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
      if(!input.name?.trim()||configName(input.name))throw Error('Invalid or duplicate orchestration.');
      const c:Config={id:crypto.randomUUID(),name:input.name,coordinatorBotId:roles.coordinator.id,
        workerBotId:b.id,workerBotIds:roles.workerBotIds,reviewerBotIds:roles.reviewerBotIds,
        teamVersion:1,projectId:b.projectId,enabled:false,policyRevision:1,
        councilRequired:!!input.councilRequired};
      configs.set(c.id,c);teamStrategies.set(c.id,defaultStrategy());value=c;break;
    }
    case 'superagent_update':{
      const c=get(args.orchestrationId),input=args.input;
      if(c.enabled||c.policyRevision!==args.expectedRevision||(tickets.get(c.id)?.length||0)>0)
        throw Error('Preset cannot change after use or while enabled.');
      const roles=resolveRoles(input),b=roles.workers[0];
      if(!input.name?.trim()||configName(input.name)?.id!==undefined&&configName(input.name)?.id!==c.id)
        throw Error('Invalid or duplicate preset.');
      c.name=input.name;c.coordinatorBotId=roles.coordinator.id;c.workerBotId=b.id;
      c.workerBotIds=roles.workerBotIds;c.reviewerBotIds=roles.reviewerBotIds;
      c.projectId=b.projectId;c.teamVersion++;c.policyRevision++;
      value=c;break;
    }
    case 'superagent_set_enabled':{
      const c=get(args.orchestrationId);if(c.policyRevision!==args.expectedRevision)throw Error('Stale policy.');
      const strategy=strategyFor(c.id);
      if(args.enabled && (strategy.maxConcurrentTasks!==1 ||
          strategy.settings.tokenCeiling!==undefined ||
          strategy.settings.estimatedCostCeilingMinor!==undefined ||
          strategy.settings.subagentDelegationEnabled))
          throw Error('Requested Team policy cannot run with current native capabilities.');
      if(c.enabled!==args.enabled){c.enabled=args.enabled;c.policyRevision++;}
      value=c;break;
    }
    case 'superagent_delete':{
      const c=get(args.orchestrationId);if(c.enabled||c.policyRevision!==args.expectedRevision||
        (tickets.get(c.id)?.length||0)>0)throw Error('Cannot delete used or enabled preset.');
      configs.delete(c.id);teamStrategies.delete(c.id);value=null;break;
    }
    case 'superagent_submit':
    case 'superagent_submit_coordinated':{
      const c=get(args.orchestrationId);if(!c.enabled)throw Error('Orchestration disabled.');
      const t:Ticket={id:crypto.randomUUID(),orchestrationId:c.id,objective:args.objective,
        state:'running',revision:0,evidence:null};
      teamSnapshots.set(t.id,makeTeamSnapshot(c));
      tickets.set(c.id,[...(tickets.get(c.id)||[]),t]);
      assignments.set(t.id,[{id:'preview-assignment-'+t.id,botId:c.workerBotId,
        sessionId:'preview-session-'+t.id,attempt:1,state:'running'}]);
      activity.set(t.id,[{id:1,kind:'admitted',detail:'Manual read-only preview assignment',at:Date.now()}]);
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
        if(t.state!=='queued')throw Error('No coordinator plan.');
        t.state='running';
      }else if(command==='superagent_retry') {
        if(t.state!=='blocked')throw Error('Ticket is not blocked.');
        t.state='running';
      }else {
        if(t.state!=='waiting_approval'||t.revision!==args.expectedRevision)throw Error('Stale worker evidence.');
        t.state=command==='superagent_accept'?'completed':'blocked';
      }
      t.revision++;
      if(command==='superagent_stop'){
        for(const entry of assignments.get(t.id)||[])if(entry.state==='running')entry.state='stopped';
      }
      const events=activity.get(t.id)||[];
      events.unshift({id:events.length+1,kind:command.slice('superagent_'.length),
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
    case 'superagent_issue_comment':{
      value={previewOnly:true,message:'Preview only. No GitHub comment was sent.'};
      break;
    }
    case 'superagent_manifest_export':{
      value={schemaVersion:1,orchestrations:[...configs.values()].map(c=>({
        name:c.name,coordinatorBot:find(c.coordinatorBotId).name,workerBot:find(c.workerBotId).name,
        enabled:c.enabled,autonomy:'off',maxConcurrentTasks:1,maxAttemptsPerAssignment:2,
        council:{enabled:false}
      }))};break;
    }
    case 'superagent_manifest_preview':
    case 'superagent_manifest_import':{
      const doc=args.document;if(doc?.schemaVersion!==1||!Array.isArray(doc.orchestrations))throw Error('Invalid manifest.');
      const pending:string[]=[],unchanged:string[]=[];
      for(const preset of doc.orchestrations){
        const found=configName(preset.name);
        if(preset.autonomy!=='off'||preset.council?.enabled||preset.maxConcurrentTasks!==1)throw Error('Unsupported policy.');
        const a=bots.filter(b=>b.name.toLowerCase()===String(preset.coordinatorBot).toLowerCase());
        const b=bots.filter(b=>b.name.toLowerCase()===String(preset.workerBot).toLowerCase());
        if(a.length!==1||b.length!==1||a[0].id===b[0].id)throw Error('Invalid portable bot reference.');
        if(found){if(found.coordinatorBotId!==a[0].id||found.workerBotId!==b[0].id)throw Error('Conflicting preset.');
          unchanged.push(preset.name);
        }else pending.push(preset.name);
      }
      if(command==='superagent_manifest_import'){
        if(args.confirmed!==true)throw Error('Confirmation required.');
        for(const name of pending){
          const preset=doc.orchestrations.find((item:any)=>item.name===name);
          const a=bots.find(b=>b.name===preset.coordinatorBot)!,b=bots.find(b=>b.name===preset.workerBot)!;
          const c:Config={id:crypto.randomUUID(),name,coordinatorBotId:a.id,workerBotId:b.id,
            workerBotIds:[b.id],reviewerBotIds:[],teamVersion:1,
            projectId:b.projectId,enabled:false,policyRevision:1};configs.set(c.id,c);
        }
        value={created:pending,unchanged};
      }else value={wouldCreate:pending,unchanged,importedPresetsDisabled:true};
      break;
    }
    default: throw Error('Unknown preview SuperAgent command: '+command);
  }
  return structuredClone(value) as T;
}
