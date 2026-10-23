> Nota de trabalho/evidência. O resultado consolidado de testes está em ../SECURITY-HARDENING.md e verification.json.

# PandaCode 0.3.28 — credenciais, saídas de dados e aprovação de bots

## Escopo e estado

Revisão da cópia imutável `audit/original/pandacode-0.3.28`, com correções pontuais em `audit/pandacode-0.3.28-hardened`. Foram examinados os prompts internos, integração Jev, ponte de CLIs de cloud, armazenamento de segredos, ferramentas e gateway dos bots, gates dos providers, HTTP compartilhado e, em segunda revisão independente, o preview de arquivos/navegador alterado pelo agente principal e as superfícies de UI rica.

Esta nota separa vulnerabilidade confirmada, endurecimento deliberado para a preferência de privacidade do usuário e limitações remanescentes. Não se encontrou um prompt interno destinado a roubar código/chaves ou um servidor oculto de coleta neste escopo. Isso não constitui garantia de ausência de malware em todo o projeto/dependências.

Os testes Rust descritos abaixo foram adicionados ao código, mas **não foram executados por este subagente**. O agente principal está responsável pelo build/testes nativos e seus resultados finais substituem este estado intermediário. Não houve envio do código/credenciais a scanners ou execução de payloads maliciosos.

## Achados e correções

### EGR-01 — Chaves Jev persistidas em JSON legível (médio; credenciais locais)

**Evidência original:** `src-tauri/src/jev.rs:56–65` inclui `typesafe_key` e `text_key` em `Saved`, com `Serialize`. `jev_save_configuration`, linhas 497–499, serializa integralmente `next` para `jev/settings.json`. A UI original, `src/jev.ts:155`, dizia expressamente que as chaves eram salvas fora do keychain. O chmod privado reduz acesso de outros usuários em Unix; não transforma o arquivo em cofre do sistema e não protege cópias/backups ou processos com o mesmo usuário.

**Pré-condição:** usuário salva ao menos uma chave Jev. Os dois recursos Jev vêm desativados por padrão (`Settings::default`, linhas 47–48). Desativar o recurso não remove uma chave já salva. A UI tem botão Remove por chave (`src/jev.ts:185–190, 233–234`), então é possível evitar o recurso e apagar os segredos manualmente.

**Correção:** `typesafe_key`/`text_key` mantêm desserialização legada, porém nunca são serializados. O JSON passa a conter somente referência UUID e flags de presença; valores ficam no `secret_store` existente, serviço `app.pandacode.jev`. Cada salvamento cria uma geração nova no cofre; a substituição atômica do JSON é o ponto de commit. Só depois do commit remove-se a geração anterior. Falha parcial de cofre ou arquivo limpa a geração nova, preservando a anterior.

O carregamento migra o JSON legado. Se o cofre estiver bloqueado/indisponível, a integração fica desativada e o arquivo original permanece recuperável. Uma proteção adicional impede que um Save posterior sobrescreva o arquivo de recuperação com defaults vazios: primeiro tenta recarregar/migrar, falhando sem modificar o original caso os segredos ainda estejam indisponíveis.

**Testes adicionados:** `legacy_credentials_move_to_the_vault_and_restart_without_plaintext`; `failed_vault_or_settings_write_preserves_previous_credential_generation`; `saving_after_unavailable_vault_preserves_the_recovery_file`; `clearing_credentials_commits_empty_refs_before_removing_old_vault_entries`.

**Limites honestos:** os testes usam o cofre em memória já adotado pelo projeto sob `cfg(test)`, não demonstram integração real com Keychain/Secret Service/Credential Manager. A remoção de entradas é best effort na API existente. A cópia de runtime `Saved` conserva os valores em RAM para a integração; a API pública `Configuration` só devolve booleans, não as chaves. Uma falha na migração deixa o arquivo legado intacto (inclusive seu plaintext) para evitar perda de segredo e desativa seu uso. Não afirmar remoção garantida de cópias antigas/backups nem ausência de plaintext em RAM.

### EGR-02 — Download/execução automática de CLI com credenciais já no ambiente (alto condicional; supply chain)

**Evidência original:** `src-tauri/src/cloud/deploy.rs:240–267`: se Vercel/Wrangler/Railway não estivessem instalados, `prepare` usava `npx --yes` com pacotes fixados nas constantes das linhas 20–22. Os tokens eram anexados ao ambiente desse mesmo subprocesso nas linhas 248, 258 e 265. Dependências/scripts de instalação executados por npx herdariam esse ambiente.

**Pré-condição:** cloud conectado, usuário aprovou a ferramenta de deploy e a CLI estava ausente. Versão fixada reduz mudança acidental, mas não elimina execução de código baixado, dependências transitivas ou scripts com acesso às credenciais. Não é evidência de pacote malicioso presente no ZIP.

**Correção:** removidas todas essas alternativas npx. A ferramenta só aceita CLI previamente instalada; ausente, retorna instrução de instalação manual de versão revisada. Nada é baixado pelo fallback da aplicação. Cloud continua opcional e a aprovação de deploy é mantida.

### EGR-03 — Overrides de destino e operações de credencial na ponte cloud (alto condicional)

**Evidência original:** os argumentos de `pandacode_cloud_cli` chegavam a Vercel/Wrangler em `command.args(args)` (`deploy.rs:248,258`) com token do cofre no ambiente; não havia filtro específico para esses dois comandos. Vercel documenta `--api` para trocar o API base URL. A proteção AWS original, linhas 88–106, só vedava `--debug` e alguns comandos de retorno de credenciais; permitia `--endpoint-url`, `--no-verify-ssl` e endpoints do ambiente/configuração.

**Impacto:** credenciais mantidas fora do prompt podiam ser usadas por uma CLI contra destino indicado nos argumentos. AWS assina requests e pode transmitir o session token; não afirmar que a secret access key de assinatura é enviada como texto em todos os casos. Operações como `vercel tokens` e `wrangler auth token` também conflitam com a promessa de não trazer credenciais ao agente; a redaction literal não cobre credenciais recém-criadas.

**Correção:** `keyed_cli_refusal` veta override de API/config/diretório, debug e operações de autenticação/token e execução local reconhecidas em Vercel/Wrangler. Deploys usuais continuam permitidos. AWS veta debug, endpoint arbitrário e desligamento de TLS, inclusive abreviações longas aceitas por argparse (`--endpoint`, `--endp=`, `--no-verify`, `--debu`); define `AWS_IGNORE_CONFIGURED_ENDPOINT_URLS=true`. O redactor genérico passa a remover todo segredo não vazio, inclusive com menos de oito caracteres.

**Teste:** `credential_bridge_refuses_endpoint_and_credential_operations` inclui formas com `=`, flags curtas anexadas, abreviações AWS, comandos de credenciais e exemplos de deploy mantidos; teste existente de scrub inclui chave curta.

**Limite:** isso é restrição da ponte privilegiada, não um sandbox completo de CLIs. CLIs instaladas, login/configuração do usuário e hooks locais de builds autorizados permanecem dependências de confiança. Um agente com execução irrestrita de shell pode tentar outros mecanismos de rede; a escolha do usuário por comandos locais completos foi preservada. A revisão não garante que todo comportamento de versões futuras de CLIs possa ser limitado por uma lista de flags.

### EGR-04 — Bot podia transmitir mensagem proativa sem aprovação (alto condicional)

**Evidência original:** `bots/tools.rs:29–30` tratava todas as ferramentas registradas de bot, incluindo `pandacode_bot_notify`, como `self_scoped`. `with_bot:44–48` não acrescentava notify à lista de aprovação. O dispatcher, linhas 73–74, chamava `gateway::notify`, que enviava ao último canal externo (`gateway.rs:326–329`). Além disso, `providers/mcp_host.rs:494` e `providers/host_tools.rs:255` só protegiam a ferramenta cloud no modo full.

**Pré-condição:** bot com destino externo configurado/último canal e execução de tarefa. Criar/parear um canal é intenção de comunicação, mas não deve tornar qualquer envio futuro de código/chave automaticamente aprovado. É possível manter canais desativados na UI, mas esse controle não corrigia o caminho de aprovação ao usá-los.

**Correção:** notify sai de `self_scoped`, entra em `approval_tools` e em `ALWAYS_ASK`; ambos os hosts exigem aprovação inclusive em full. Guard de bot não permite que uma regra ampla ou uma resposta “always” memorize permissão permanente para notify/cloud_cli. Memória e operações locais do bot preservadas. Falha de transmissão passa a retornar erro ao chamador, em vez de informar `sent=true`.

**Testes:** `outbound_tools_ask_even_in_full_mode` em MCP e host_tools verifica handler não chamado antes da decisão; negativa MCP não executa. `outbound_bot_tools_cannot_be_covered_by_broad_allow_rules` verifica precedência da proteção. O teste `only_registered_bot_tools_skip_approval` foi ajustado para a nova fronteira.

### EGR-05 — Aprovação mandava o conteúdo sensível antes da autorização (alto condicional)

**Evidência original:** `bots/gateway.rs:284–288` envia os primeiros 600 caracteres de `text` (descrição/argumentos da ação) ao canal externo como pedido de aprovação. Um comando/texto contendo segredo já sairia do PC antes que uma recusa pudesse impedir a ação.

**Correção:** mensagem externa de aprovação agora é fixa e sem nome/argumentos/payload: solicita revisar a ação completa no desktop e responder sim/não para aquela ação. A UI local conserva os detalhes para decisão. As mensagens de resultado de uma conversa externa iniciada pelo usuário continuam funcionando.

### EGR-06 — Resultados agendados eram encaminhados automaticamente (mudança de privacidade deliberada)

**Evidência original:** `bots/gateway.rs:194–200` chamava `notify` com o texto final de trabalho agendado; notify o mandava ao último canal externo. Era um comportamento funcional documentado, não indício de malware.

**Correção alinhada ao pedido:** resumo agendado aparece no desktop, via `notify_local`. Para transmissão externa proativa usa-se notify com aprovação explícita. Preservadas agendas e execução local. Notificação genérica de ação pendente pode continuar indo ao canal, sem o conteúdo da ação.

### EGR-07 — Canal revogado não era revalidado pelo gateway (médio/alto condicional)

**Evidência original:** `bots/gateway.rs:91–101` verificava pairing do remetente, mas não `channel.enabled` e associação do canal ao bot. Um evento já obtido pelo poller podia ser admitido depois da revogação.

**Correção:** `validate_channel` verifica existência, enabled e propriedade no ingresso e de novo antes de compor/enfileirar a tarefa. Desktop e agendamento internos têm tratamento explícito. Agente de remote implementou em paralelo a verificação imediata em `channels::send`; o erro se propaga por notify, evitando falso sucesso. Controle de rede em trânsito não pode retirar um pacote já enviado antes da revogação.

**Teste:** `bot_admission_rechecks_channel_revocation_and_ownership` cobre canal habilitado, bot diferente, disable e desktop.

### EGR-08 — Limite HTTP verificado após alocar corpo completo (médio; disponibilidade)

**Evidência original:** `cloud/http.rs:40–45` verifica Content-Length quando presente, mas chama `response.bytes()` antes de verificar tamanho real. Um endpoint configurado ou resposta maliciosa chunked sem Content-Length podia induzir alocação excessiva.

**Correção:** leitura por `response.chunk()`, conferindo soma saturada contra 16 MiB antes de estender o buffer. Mantém verificação preliminar de Content-Length.

**Teste:** `rejects_oversized_chunked_responses_without_content_length`, servidor exclusivamente loopback, envia corpo chunked excessivo e exige recusa. Não usa servidor externo.

## Fontes primárias para características de CLIs

- Vercel global options: https://vercel.com/docs/cli/global-options e referência oficial https://github.com/vercel/vercel/blob/main/skills/vercel-cli/references/global-options.md (`--api`, config/cwd/debug).
- Vercel tokens: https://vercel.com/docs/cli/tokens .
- Cloudflare Wrangler comandos gerais: https://developers.cloudflare.com/workers/wrangler/commands/general/ (`auth token` e `--json`).
- AWS endpoints: https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-endpoints.html .
- AWS opções abreviadas: https://docs.aws.amazon.com/cli/latest/userguide/cli-chap-troubleshooting.html , seção sobre incomplete parameter names/argparse `allow_abbrev`.
- Claude env vars (recomendação encaminhada ao agente de updates): https://code.claude.com/docs/en/env-vars .

## Fluxos examinados que não foram classificados como coleta oculta

- `lib.rs:447–459` e prompts auxiliares orientam título, run scripts, resumo de mudanças e UI rica. Não foram encontradas instruções de envio a domínio secreto nesses prompts.
- `bots/brief.rs:47–50` pede aprovação antes de envio externo, proíbe revelar tokens/chaves e manda tratar páginas/arquivos como dados. O problema confirmado estava em enforcement, não em instrução maliciosa nesse texto.
- Provedores selecionados (incluindo GLM/MiMo) e modelo textual Jev são integrações de IA explícitas; tráfego de contexto ao provedor escolhido é esperado pelo usuário. O texto/modelo-base Jev é configurável, com validação HTTPS; não confundir uma escolha do usuário com endpoint oculto.
- `support.rs:6` aponta para issues do GitHub; abertura é iniciada pelo usuário e os detalhes preparados são versão/plataforma/arquitetura, não upload silencioso do repositório.
- `code_search.rs:46–49` executa codedb local com `--no-telemetry`, `CODEDB_NO_TELEMETRY=1` e `CODEDB_NO_CLI_DAEMON=1`.
- Chaves cloud existentes usam `secret_store`; a descoberta de Jev não deve ser generalizada incorretamente para todas as credenciais da aplicação.

## Segunda revisão independente do preview e UI rica

Estado revisado: `file_preview.rs` e `browser.rs` alterados pelo agente principal, sem editar esses arquivos.

- `file_preview.rs:23–25`: CSP distinto para arquivos e bootstrap. Arquivos têm `script-src 'none'`, `sandbox allow-same-origin`, connect/worker/object/frame/form externos bloqueados. O bootstrap permite só o próprio JS gerado para adquirir o cookie Strict e redirecionar.
- `file_preview.rs:559–568`: caminho é reconstruído por URL e seu path serializado em JSON; o corpo não interpola HTML de workspace. Busca/hash são concatenados no runtime como strings. Não foi identificada injeção no script por nome de arquivo nessa implementação.
- Headers incluem nosniff, no-store, CORP same-origin, X-Frame-Options DENY, no-referrer e `X-DNS-Prefetch-Control: off`. Host/Origin/Fetch-Metadata/referrer e cookie continuam controlados; symlinks são canonicalizados dentro do workspace.
- `browser.rs:47–56,257–279`: `preview_origin` é ajustado pela chamada UI confiável `browser_open`, e `on_navigation` rejeita mudança de origem enquanto preview estiver aberto. Popups e downloads seguem negados. Não foi encontrada regressão de segurança bloqueante por inspeção estática; cobertura real depende dos motores nativos e dos testes do agente principal.
- Rich markdown/HTML usa DOMPurify; HTML retira link/meta/base; custom views retiram recursos e sanitizam CSS. UI nativa CSP restringe URLs de SVG/CSS que não forem eliminadas pelo sanitizador. Design canvas usa srcdoc bruto com iframe sandbox sem scripts e herda CSP da aplicação. Não se confirmou outro egress de alta severidade nesses fluxos.
- Sugerido ao principal, como defesa adicional sem alegar reprodução de vazamento: meta `x-dns-prefetch-control=off` cedo no HTML principal para impedir resolução especulativa de links/Design em motores que a habilitem. Fonte primária Chromium: https://www.chromium.org/developers/design-documents/dns-prefetching/ .

## Arquivos modificados por este subagente

`src-tauri/src/jev.rs`; `src-tauri/src/cloud/deploy.rs`; `src-tauri/src/cloud/http.rs`; `src-tauri/src/bots/tools.rs`; `src-tauri/src/bots/gateway.rs`; `src-tauri/src/bots/guard.rs`; trechos de `src-tauri/src/bots/tests.rs`; gates/testes específicos de `src-tauri/src/providers/mcp_host.rs` e `src-tauri/src/providers/host_tools.rs`.

Mudanças de canais/remote, child_env/updaters, frontend Jev, navegador/preview, dependências e documentação final pertencem aos outros agentes. Nenhum commit ou envio remoto foi criado por este subagente.

## Complemento solicitado: provisionamento AWS e DNS especulativo

O agente principal acrescentou `src-tauri/src/cloud/aws.rs` e o meta DNS de `index.html` ao escopo depois da revisão acima.

### EGR-09 — Senha do guest em tracing e arquivo temporário permissivo (médio; segredo de VM opcional)

**Evidência original:** `cloud/aws.rs:51` emitia `set -eux`. O `printf` da linha 56 incluía o password em argumento de shell, que `-x` imprime no stderr/log cloud-init. `create:178–179` escrevia user-data (contendo o password) por `std::fs::write` em diretório temporário: criação dependia do umask e poderia truncar um caminho preexistente. A remoção após `self.call` já ocorria para o resultado comum de sucesso/erro; faltava um objeto de duração que garantisse tentativa de limpeza nos demais retornos/unwind.

**Correção:** prologue Linux usa `set +x` antes do segredo, depois `set -euo pipefail`. `umask 077` entra depois da instalação apt, para proteger os arquivos VNC sem alterar a criação de arquivos pelos pacotes do sistema. `UserDataFile` cria o arquivo com `create_new(true)` e `mode(0o600)` em Unix antes de escrever, sincroniza e fecha o handle. Só constrói o guard depois da criação bem-sucedida, portanto uma colisão/symlink não é sobrescrita nem removida. `Drop` remove o arquivo após uso, inclusive no caminho de erro da chamada AWS; o chamador também dá drop explicitamente logo após `call`.

O comando AWS interno de provisionamento também define `AWS_IGNORE_CONFIGURED_ENDPOINT_URLS=true`, como a ponte de deploy: o upload de user-data não deve ser redirecionado silenciosamente por endpoints herdados do ambiente/configuração da CLI.

**Testes adicionados:** `user_data_file_is_private_exclusive_and_removed_after_use`, `user_data_file_is_removed_when_the_caller_fails`, `user_data_file_never_follows_or_removes_an_existing_symlink` (Unix), `linux_user_data_disables_inherited_tracing_before_secrets` (Unix). O último executa só o prologue gerado mais um printf para `/dev/null` sob `bash -x`, sem apt, rede ou provisionamento, e exige ausência do segredo no stderr.

**Limites:** o guard tenta remoção durante fluxo normal/unwind; não promete limpeza após crash abrupto/kill/power loss nem apagamento forense de disco. Em Windows, herda as ACLs do diretório temporário do usuário; a mudança 0600 é específica Unix. User-data EC2 ainda leva o segredo ao guest que o usuário explicitamente mandou provisionar. Não é criptografia de user-data, da VM ou de backups AWS.

### EGR-10 — MSI TightVNC era executado sem validar assinatura (alto condicional; VM Windows)

**Evidência original:** `cloud/aws.rs:92–94` baixava `tightvnc-2.8.90-gpl-setup-64bit.msi` e imediatamente executava msiexec como administrador, sem hash/assinatura; usava nome fixo no TEMP e não removia o instalador. A senha Administrator e o autologon eram definidos antes de saber se a instalação funcionaria.

**Pesquisa e escolha:** a página oficial atual confirma a versão 2.8.90 e instalador x64 de 2.543.616 bytes, https://www.tightvnc.com/download/ . Não foi possível obter um binário verificável do ambiente da auditoria: a tentativa anterior do principal recebeu HTML de erro; a deste subagente foi impedida pela rede do sandbox. Não se usou hash de página de erro ou hash de agregador como âncora. O mantenedor Constantin Kaplinsky instrui, na lista oficial, a conferir assinatura válida de “GlavSoft LLC” ou “GlavSoft OOO” em vez de checksum: https://sourceforge.net/p/vnc-tight/mailman/vnc-tight-list/?limit=25&page=7 , mensagem “Re: Checksums for latest installers?”, 23/03/2021. A página oficial https://www.tightvnc.com/announce-2.0beta2.php também identifica assinatura GlavSoft LLC.

**Correção escolhida:** assinatura Authenticode em modo fail closed, conforme alternativa autorizada pelo principal. Mantém URL/versionamento 2.8.90 e criação explícita da VM. Cria um staging aleatório com DACL sem herança e acesso apenas a SYSTEM/Administrators. Download tem timeout de 120s e nenhum redirect permitido. `Get-AuthenticodeSignature` deve retornar `Status=Valid` e certificado de assinante existente; `GetNameInfo(SimpleName,false)` deve bater exatamente com `GlavSoft LLC`, `GlavSoft LLC.` ou `GlavSoft OOO`, sem wildcard. Só então executa msiexec de `System32`; aceita códigos 0/3010. `finally` remove staging e download em sucesso/falha. A senha Administrator/autologon é configurada somente após instalação bem-sucedida e cleanup.

Documentação Microsoft do verificador: https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/get-authenticodesignature?view=powershell-5.1 . A assinatura valida origem/integridade segundo a confiança de certificados do guest, não demonstra que o programa não possui vulnerabilidades ou que o publisher nunca foi comprometido. Não há checksum fixado nesta alternativa.

**Teste adicionado:** `windows_verifies_publisher_before_install_and_cleans_up_before_autologon` verifica o contrato do script emitido: ordem dos checks, publisher exato, execução somente depois dos checks, bloco finally e autologon depois da limpeza. O teste é estrutural e não substitui execução de PowerShell/Authenticode no Windows. O fluxo Windows/VM não foi executado neste ambiente; falha de download/assinatura aborta a instalação, sem um fallback inseguro.

**Segredos mantidos por desenho explícito:** para abrir desktop Windows sem lockscreen, o projeto continua configurando Administrator e Winlogon AutoAdminLogon, incluindo `DefaultPassword` no registro **do guest AWS**. VNC usa autenticação tradicional de oito caracteres, restrita a loopback e túnel SSM; esse password também aparece nos parâmetros da instalação dentro do guest. Esses fatos precisam constar na orientação do usuário: não usar essa VM como cofre de segredos, controlar IAM/SSM e encerrar a VM quando dispensável. A alteração evita o log acidental no Linux e melhora o armazenamento temporário local; não elimina todo segredo do guest nem altera o modelo de acesso cloud.

`run-instances` não escolhe um security group próprio: a política do default SG/VPC da conta continua valendo, inclusive possíveis regras intra-SG. O comentário antigo prometia “no inbound ports”; foi ajustado para refletir só o que o código implementa: não cria key pair SSH nem adiciona regra de ingresso VNC, que fica em loopback. Não houve criação ou alteração de recursos AWS reais nem de regras de rede da conta nesta auditoria.

### EGR-11 — Defesa adicional contra DNS especulativo

`index.html` agora contém `<meta http-equiv="x-dns-prefetch-control" content="off">` imediatamente após o charset, antes de links/renderização. Impacto esperado: desabilitar resolução especulativa de hosts, sem impedir links explicitamente abertos. Chromium documenta que opt-out explícito não pode ser reabilitado depois pelo conteúdo e que frames filhos herdam o opt-out: https://www.chromium.org/developers/design-documents/dns-prefetching/ . Trata-se de hardening adicional, não de vazamento reproduzido na aplicação original.

**Verificação executada:** foi extraído do próprio `aws.rs` o prologue Linux emitido e executado sob `bash -x` com apenas um printf de fixture para `/dev/null`. Passou: código de saída zero, stdout vazio e nenhuma ocorrência da fixture no stderr. Nenhum comando de provisionamento foi executado. O diff de aws.rs também não apresentou alertas de whitespace.

**Estado final deste subagente:** edições e testes adicionados; sem compilação/testes Rust paralelos e sem build npm. Resultados de execução nativa serão apresentados pelo agente principal.
