/** Deterministic preview-only SuperAgent adapter. No real harness or external writes. */
type Bot = { id:string; name:string; paused:boolean; projectId:string };
type Config = { id:string; name:string; coordinatorBotId:string; workerBotId:string;
  projectId:string; enabled:boolean; policyRevision:number };
type Ticket = { id:string; orchestrationId:string; objective:string;
  state:string; revision:number; evidence:null };
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
    case 'superagent_list': value=[...configs.values()];break;
    case 'superagent_tickets':value=tickets.get(args.orchestrationId)||[];break;
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
      tickets.set(c.id,[...(tickets.get(c.id)||[]),t]);value=t;break;
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
      value=t;break;
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
