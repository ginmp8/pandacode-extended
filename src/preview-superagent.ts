/** Deterministic preview-only SuperAgent adapter. No real harness or external writes. */
type Bot = { id:string; name:string; paused:boolean; projectId:string };
type Config = { id:string; name:string; coordinatorBotId:string; workerBotId:string;
  projectId:string; enabled:boolean; policyRevision:number };
type Ticket = { id:string; orchestrationId:string; objective:string;
  state:string; revision:number; evidence:null };
type Activity = { id:number; kind:string; detail:string; at:number };
type Assignment = { id:string; botId:string; sessionId:string; attempt:number; state:string };
const activity = new Map<string,Activity[]>(), assignments = new Map<string,Assignment[]>();
const threadLinks = new Map<string,Array<{sourceId:string;ticketId:string;repository:string;subjectKey:string;createdAt:number}>>();
const configs = new Map<string,Config>(), tickets = new Map<string,Ticket[]>();
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
  let value:unknown;
  switch(command){
    case 'superagent_list': value=[...configs.values()].map(config=>({
      ...config,
      ticketCount:(tickets.get(config.id)||[]).length,
      runningCount:(tickets.get(config.id)||[]).filter(ticket=>ticket.state==='running').length,
      blockedCount:(tickets.get(config.id)||[]).filter(ticket=>ticket.state==='blocked').length,
      lastActivityAt:Date.now(),
    }));break;
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
      else value={ticket:t,mandate,assignments:assignments.get(t.id)||[],activity:activity.get(t.id)||[]};
      break;
    }
    case 'superagent_source_thread_links':
      value=threadLinks.get(args.ticketId)||[];break;
    // These are preview-only references. Real native code verifies source and
    // Ticket orchestration ownership and rejects conflicting thread reuse.
    case 'superagent_source_link_thread':
      throw Error('Source linking requires a native trusted event source.');
    case 'superagent_create':{
      const input=args.input;const a=find(input.coordinatorBotId),b=find(input.workerBotId);
      if(!input.name?.trim()||a.id===b.id||configName(input.name))throw Error('Invalid or duplicate orchestration.');
      const c:Config={id:crypto.randomUUID(),name:input.name,coordinatorBotId:a.id,
        workerBotId:b.id,projectId:b.projectId,enabled:false,policyRevision:1};
      configs.set(c.id,c);value=c;break;
    }
    case 'superagent_update':{
      const c=get(args.orchestrationId),input=args.input;
      if(c.enabled||c.policyRevision!==args.expectedRevision||(tickets.get(c.id)?.length||0)>0)
        throw Error('Preset cannot change after use or while enabled.');
      const a=find(input.coordinatorBotId),b=find(input.workerBotId);
      if(!input.name?.trim()||a.id===b.id||configName(input.name)?.id!==undefined&&configName(input.name)?.id!==c.id)
        throw Error('Invalid or duplicate preset.');
      c.name=input.name;c.coordinatorBotId=a.id;c.workerBotId=b.id;c.projectId=b.projectId;c.policyRevision++;
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
      const t:Ticket={id:crypto.randomUUID(),orchestrationId:c.id,objective:args.objective,
        state:'running',revision:0,evidence:null};
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
