> **Relatório histórico de hardened.1.** Não representa os testes da revisão atual.
> As revisões seguintes estão em `SECURITY-INSTALLATION-REVIEW.md` (hardened.2) e `security-review/hardened.3/README.md` (hardened.3).
> A revisão de instalação, as correções e os limites de hardened.2 estão em
> [SECURITY-INSTALLATION-REVIEW.md](SECURITY-INSTALLATION-REVIEW.md).
> Os antigos manifestos, patch e resumo de verificação estão em `security-review/hardened.1/`; os demais logs antigos permanecem em `security-review/`.

# Revisão de segurança — PandaCode 0.3.28 / hardened.1

Data: 23 de outubro de 2026. Idioma do relatório: português brasileiro.

This is a fork of [PandaCode](https://github.com/aicodeking/pandacode). A licença, os créditos, a arquitetura e os recursos do projeto original foram preservados. `hardened.1` identifica esta distribuição revisada; o número interno 0.3.28 permanece para compatibilidade.

## Resultado

**A cópia original não atende ao requisito de evitar atualizações externas automáticas e reduzir saídas de dados sem autorização específica.** Foram encontrados caminhos concretos de envio prematuro por bots, problemas de revogação/repetição remota, armazenamento inadequado de chaves opcionais, instalação automática com credenciais no ambiente e dependências com correções de segurança disponíveis. Alguns comportamentos eram funcionalidades documentadas; não são, por si, prova de malware.

As correções estão nesta própria árvore, baseada no ZIP recebido. A revisão não encontrou uma instrução interna oculta que ordenasse à IA roubar código, chaves ou dados para um servidor de coleta separado. Também não confirmou credencial real embutida nos arquivos próprios examinados. Esses resultados descrevem o escopo analisado e não constituem certificação de ausência de malware ou de vulnerabilidades desconhecidas.

O uso dos modelos escolhidos, a execução de comandos, Git, equipes, tarefas, terminal, deploys explícitos e integrações opcionais foram preservados. A distribuição não é um firewall nem um sandbox adicional para todo comando que a IA possa executar.

## Origem e escopo

| Item | Registro |
| --- | --- |
| Arquivo recebido | `pandacode-0.3.28.zip` |
| SHA-256 original | `4d74c67ab175967fa247852cd8d678e3908cb51de9d5a9292b1dd5dc6d500836` |
| Inventário original | 888 entradas no ZIP, 820 arquivos; extração sem caminhos de fuga ou links simbólicos |
| Projeto | Aplicativo desktop Tauri 2/Rust, frontend TypeScript/Vite; fontes do relay, website e instaladores incluídas |
| Revisão | Fluxos de credenciais, prompts internos, chamadas externas, atualização/instalação, IPC, navegador e prévias, Connect, bots/webhooks, nuvem, limites de recursos e dependências fixadas |
| Registro de mudanças | `security-review/hardened.1/changes.patch` e `security-review/hardened.1/manifest.json` |
| Evidências | `security-review/hardened.1/verification.json`, logs e notas detalhadas por subsistema |

A análise combinou inspeção de código, comparação com uma cópia imutável, consultas a advisories oficiais, regressões executáveis, testes existentes e revisão independente das mudanças de maior risco. As consultas de dependências enviaram somente nomes e versões públicos; o repositório e as credenciais do usuário não foram enviados a scanners externos. Não foram conectadas contas pessoais nem usados modelos pagos para os testes.

## Problemas encontrados e tratamento

As gravidades abaixo consideram condições reais de uso. “Alta condicional” significa que é necessário ativar uma integração ou abrir conteúdo específico; não significa acesso remoto anônimo disponível na instalação padrão.

| Área / gravidade | Evidência no original e consequência | Tratamento nesta revisão |
| --- | --- | --- |
| Atualização do app — requisito de privacidade | `src/app-updates.ts` iniciava checks, download e instalação automática; `tauri.conf.json` e capabilities expunham updater/restart. Havia opção de desligar na UI, mas dependia da preferência. | Controlador inerte; plugin, dependências, registro nativo, endpoint e capacidades removidos. Preferências antigas não reativam o mecanismo. |
| Atualização dos agentes — requisito de privacidade | Controlador fazia verificações periódicas; comandos nativos ofereciam instalação/atualização. | Polling e instalação desativados no frontend e nos pontos de entrada nativos. Opt-outs documentados são passados aos CLIs que os suportam. |
| npx / continuidade da versão — requisito de privacidade | O instalador incluído baixava e instalava a versão upstream mais recente. Isso substituiria as correções desta distribuição. | A cópia incluída recusa download, instalação e execução, inclusive em dry-run. Instalação desta revisão exige build do código fornecido. |
| Dependência Tauri — alta | `Cargo.lock` fixava Tauri 2.11.5, abrangido pelo GHSA-w28w-mhc8-qvjv: isolamento insuficiente na recuperação de mensagens de canal entre webviews. | Tauri fixado em 2.11.6, versão corrigida. O guest continua sem permissões de IPC do aplicativo. |
| Prévia HTML — alta condicional | `file_preview.rs` servia HTML/JS do workspace com CSP somente de frame-ancestors. Um arquivo aberto podia executar scripts, ler recursos permitidos daquela origem e tentar transmiti-los. O bloqueio de `.env` não cobria todo arquivo potencialmente confidencial. | Arquivos recebem sandbox sem scripts; fetch, workers, forms, frames, objetos e rede externa são bloqueados. Navegação do guest fica restrita à origem da prévia. HTML/CSS/imagens locais continuam visíveis. |
| Revogação Connect — alta condicional | A fila conservava confiança antiga; operações lentas podiam prosseguir depois da revogação. Havia janela de troca de chave e envio após perda de confiança. | Revalidar chave autenticadora, pin, confiança e hosting ao despachar, após preparação lenta e antes de respostas/eventos. |
| Replay Connect — média/alta condicional | Timestamp extremo podia causar overflow; responses/events não tinham metadata antirrepetição; cache de requests era volátil. | Aritmética segura; ts/nonce dentro de todos os corpos cifrados; recibos persistentes antes da admissão, com fail-closed e proteção contra relógio retrocedido. |
| Notify de bot — alta condicional | `pandacode_bot_notify` era self-scoped e podia mandar texto proativo sem uma aprovação específica, inclusive em Full access. | Notify sempre pede aprovação; regras amplas e “always” não tornam a autorização permanente. |
| Pedido de aprovação externo — alta condicional | `bots/gateway.rs` enviava ao canal os primeiros 600 caracteres da ação antes de o usuário aprová-la. | O canal recebe aviso fixo, sem argumentos/conteúdo da ação. Detalhes permanecem no desktop. |
| Resultados agendados — preferência de privacidade | Resultado de agenda era encaminhado automaticamente ao último canal externo. Era comportamento funcional, não instrução maliciosa. | Resumo fica no desktop. Envio externo proativo usa a ferramenta com aprovação. |
| Canal/link revogado — média | Eventos já coletados e long polls podiam continuar depois de desabilitar canal ou renovar token. | Revalidar canal, propriedade e token na admissão, na espera e antes de cada parte enviada. Falha não informa falso sucesso. |
| WhatsApp — média | Webhook assinado não tinha deduplicação persistente, idade e vínculo ao número configurado suficientes. | Conferir phone_number_id, ID e timestamp, gravar recibo antes da admissão e falhar fechado em capacidade excedida. |
| Jev — média, credencial local | Chaves opcionais eram serializadas em `jev/settings.json`; permissão privada não equivalia a cofre. | Migrar para o cofre do sistema com referência UUID, commit atômico e rollback. Falha preserva recuperação e bloqueia uso/salvamento destrutivo. |
| CLI de deploy — alta condicional | Se ausente, Vercel/Wrangler/Railway eram executados via `npx --yes` já com tokens no ambiente. Código baixado e scripts herdariam os tokens. | Exigir CLI previamente instalada; remover download automático dessa ponte privilegiada. |
| Endpoint/credencial cloud — alta condicional | Argumentos como Vercel `--api` e AWS `--endpoint-url`/`--no-verify-ssl` permitiam redirecionar uso das credenciais ou retirar proteção TLS. | Bloquear opções sensíveis, operações de credenciais e abreviações AWS; ignorar endpoints AWS herdados da configuração. |
| Limites HTTP — média, disponibilidade | Alguns corpos eram limitados depois de `bytes()`, permitindo alocação excessiva em resposta chunked; HTTP de bots aceitava framing ambíguo. | Limitar durante streaming; teto de mensagem WebSocket e conexões HTTP; rejeitar duplicação de cabeçalhos e framing não suportado. |
| AWS user-data — média condicional | `set -eux` expunha a senha VNC em trace; arquivo temporário local era criado sem exclusividade/permissão explícita. | Desligar xtrace antes de segredos; arquivo exclusivo, 0600 em Unix, guard de limpeza no sucesso/erro, recusa de symlink/preexistente. |
| TightVNC no guest AWS — média condicional | MSI externo era executado sem verificar assinatura/publisher. | Staging privado; Authenticode válido e publisher GlavSoft esperado antes do msiexec; sem redirects; cleanup e verificação de exit code. |
| Desktop de VM — preferência de instalação | Criar/iniciar Linux VM podia instalar desktop/pacotes em background. | Instalação somente pela ação explícita Add a desktop. |
| Preços e DNS — redução de tráfego | Estimativa fazia GET diário da tabela pública; links podiam permitir DNS especulativo dependendo do motor. | Usar preços salvos/embutidos, sem refresh; DNS prefetch desativado no app e na prévia. |

As notas `notes-egress.md`, `notes-remote.md` e `notes-updates.md` registram arquivos, linhas originais, funções, condições, testes e decisões com mais detalhe. São notas de trabalho; a tabela de validação abaixo é o resultado consolidado final.

### Por que apenas desligar opções da UI não bastava

Desligar o updater do app e deixar Connect, Jev ou canais desconectados evitaria parte dos caminhos opcionais. Isso não corrigiria a biblioteca Tauri vulnerável, a prévia ativa de arquivos, os limites de leitura nem os defeitos que aparecem quando as integrações são usadas. Por isso foi gerada uma versão corrigida, em vez de classificar o ZIP original como seguro sem alterações.

## Dependências e proveniência

| Componente | Original | Nesta revisão | Motivo / evidência |
| --- | --- | --- | --- |
| Tauri | 2.11.5 | 2.11.6 | GHSA-w28w-mhc8-qvjv, isolamento entre webviews |
| DOMPurify | 3.4.15 no lock | 3.4.17 | GHSA-mrv2-3pqr-j5jr e GHSA-cg2g-xh64-v4q7; condições específicas não demonstradas no app, atualização preventiva |
| KaTeX | 0.16.47 | 0.18.2 | GHSA-238p-pmpm-9mq7; exploração exige poluição prévia de protótipo, não demonstrada aqui |
| Vite | 7.3.6 | 7.3.7 | Correções da linha 7 para servidor de desenvolvimento/Windows; não confundir dev server com servidor exposto por padrão |
| source-map-js | 1.2.1 | 1.2.2 | GHSA-68fv-2mgg-jv7q, consumo de CPU com source map malicioso |
| rustls | 0.23.45 | 0.23.46 | RUSTSEC-2026-0346 a 0349; várias condições específicas não habilitadas nesta configuração |
| rustls-webpki | 0.103.15 | 0.103.16 | RUSTSEC-2026-0382, recusa de intermediários emSign desautorizados |
| webpki-roots / root-certs | 1.0.9 | 1.0.10 | RUSTSEC-2026-0381/0383, trust store; wrapper 0.26.11 resolve o pai corrigido |
| zip | 2.4.2 | 9.0.0 | RUSTSEC-2026-0367/0368/0369; APIs de extração não usadas pelo app, mas o código vulnerável também foi retirado da árvore resolvida |
| glib | 0.18.5 | 0.18.5 + backport local | RUSTSEC-2024-0429: duas linhas da correção oficial, mantendo tipos GTK3 compatíveis |

O backport GLib preserva licença e versão autêntica, com hashes e verificador em `src-tauri/vendor/glib/BACKPORT.md` e `BACKPORT_PROVENANCE.json`. A versão não foi falsificada para silenciar ferramentas. O teste otimizado de iteração cobre o comportamento que originou o advisory. A mudança ZIP mantém o consumidor VSIX e exige Rust 1.88 ou superior.

**npm:** a consulta final do lock raiz retornou zero vulnerabilidades conhecidas. O resultado não cobre automaticamente pacotes que o usuário instalar depois, CLIs externos nem todos os projetos opcionais em subpastas.

**Rust:** foram consultados 591 pacotes de registry e o GLib local pelo nome/versão, totalizando 592 consultas. Os advisories de TLS e ZIP deixaram de aparecer. O número GLib ainda é sinalizado pelo banco público e está resolvido pelo backport verificável. Permanecem nove avisos de manutenção: `proc-macro-error`, `unic-char-property`, `unic-char-range`, `unic-common`, `unic-ucd-ident`, `unic-ucd-version`, `zbus_names`, `zvariant` e `zvariant_derive`. São dívida de manutenção de dependências transitivas; não foram reclassificados como exploits comprovados nem escondidos.

## Validação realizada

Ambiente: Linux x86_64, Node 24.19.0, Rust/Cargo 1.99.1, bibliotecas GTK/WebKit e navegadores de teste isolados. As dependências npm foram instaladas com scripts de instalação desabilitados. Os testes de UI usam dados locais simulados. Os testes HTTP relevantes usam loopback.

| Verificação | Resultado consolidado |
| --- | --- |
| TypeScript + bundle de produção | Passou: TypeScript e bundle final Vite |
| Testes Node do app | 126 passaram; zero falhas |
| Testes dos scripts de release / instalador npx | 39 passaram; zero falhas |
| Backend Rust completo | 1.189 passaram; zero falhas; 39 ignorados explicitamente |
| UI Chromium, dez arquivos afetados | 95 cenários aprovados em duas execuções: 90 na suíte inicial e os cinco de VM após corrigir a plataforma simulada do fixture (MacIntel para Tart); zero falhas remanescentes |
| Servidor nativo de prévias + Chromium/WebKit | Passou: um fixture nativo executou ambos os motores, incluindo scripts/fetch bloqueados, HTML/CSS/SVG, cookies, reload e restauração |
| GLib + compatibilidade ZIP, otimização release | 6 passaram; zero falhas |
| Recibos remotos persistentes, módulo de produção isolado | 4 passaram; zero falhas; também integrados à suíte nativa |
| Provas comparativas de parser/timestamp | Original falha, correção passa; overflow também exercitado com otimização |
| Proveniência GLib | 121 arquivos upstream verificados; somente o arquivo do patch diverge nas duas linhas documentadas |
| Segredos hardcoded | 591 arquivos próprios; zero credenciais reais confirmadas após triagem; relatório sanitizado incluído |
| npm audit / OSV | Resultados JSON incluídos, com as ressalvas acima |

Os testes ignorados não foram contados como sucesso. Incluem integrações que dependem de CLIs autenticadas, plataformas ou serviços externos. A prévia real de arquivos foi exercitada separadamente de forma explícita quando indicada na tabela. Os logs, comandos e resultados estão em `security-review/`; não basta a presença de um teste no código para que ele seja tratado como executado.

## Comportamento e compatibilidade

### Saídas de dados que continuam existindo por escolha do usuário

| Recurso escolhido | Dados/destino | Como evitar quando não desejado |
| --- | --- | --- |
| Provedor de IA/CLI | Prompt, anexos, contexto e resultados de ferramentas ao provedor configurado | Escolher o provedor/endpoint e limitar contexto/permissões conforme a tarefa |
| Jev | Objetivo/texto visível e, na assistência, trechos selecionados/diffs a TypeSafe e ao modelo configurado | Manter os recursos Jev desligados |
| Connect | Login GitHub entrega token ao relay configurado para troca; depois relay encaminha tráfego cifrado entre dispositivos | Não entrar; se conectado, usar Sign out |
| Bots/canais | Resposta à conversa externa escolhida segue para o canal; notify proativo pede aprovação | Desabilitar/remover canal e renovar link quando necessário |
| Deploy/nuvem | Código/artefatos e credenciais de uso ao serviço escolhido, sob aprovação | Não conectar/provisionar/deployar o serviço |
| Navegador, Git/GitHub, shell | Tráfego dos sites, repositórios e comandos explicitamente utilizados | Controlar essas ações na tarefa; são funções preservadas do orquestrador |

O app não substitui nem altera as credenciais do provedor por um endpoint de coleta. A revisão não promete que código enviado intencionalmente ao modelo ficará exclusivamente no PC.

### Mudanças intencionais de conveniência

- Atualizações do app/agentes e o instalador npx incluído ficam desativados no código, sem interruptor para reativar.
- Estimativas usam preços locais, que podem estar desatualizados.
- Prévia de arquivo não executa JavaScript. Para testar um app interativo, abrir explicitamente seu servidor de desenvolvimento no navegador continua possível.
- Resumos agendados não são enviados automaticamente a canais externos; notify pede uma nova aprovação.
- CLIs de deploy precisam ser instalados manualmente antes do uso; desktop Linux de VM requer ação explícita.
- Connect requer hardened.1 nos dois computadores. Frames antigos sem metadata são recusados.

## Limites materiais

1. **Plataformas:** foi compilado/testado o backend Linux e o frontend. Não foi produzido nem validado um instalador Windows/macOS nesta revisão. O fluxo PowerShell/AuthentiCode do guest AWS e os cofres reais das três plataformas não foram testados em seus sistemas nativos; os testes de cofre usam a implementação de teste do projeto.
2. **CLIs externos:** flags/variáveis de opt-out não são bloqueio de rede. Um CLI, plugin, configuração própria ou comando autorizado pode fazer downloads/envios. Configurações internas do fornecedor podem sobrepor variáveis herdadas. Os executáveis externos não estão contidos nem foram integralmente auditados neste ZIP.
3. **Armazenamento de chaves:** chaves necessárias ao runtime existem em RAM. Migração Jev com cofre indisponível preserva o arquivo legado, inclusive plaintext, para recuperação e bloqueia uso. Backups antigos não são apagados retroativamente.
4. **Connect:** queda após gravar recibo e antes de executar pode perder uma mensagem; a prioridade é não executar duas vezes. Relógio que retrocede bloqueia até o watermark. Falha ao abrir o ledger requer correção/reinício. A proteção não resiste à restauração deliberada de todo o diretório de dados por seu proprietário/admin, nem impede atraso/descarte/reordenação da primeira entrega pelo relay. Revogar acesso não desfaz trabalho já iniciado.
5. **AWS opcional:** senha do guest aparece em user-data e o Windows guest usa autologon/DefaultPassword no registro; o MSI recebe a senha VNC na instalação. Isso não significa criptografia geral dos segredos do guest. VNC fica em loopback e usa SSM, mas o security group padrão da conta continua determinando outros ingressos. O download MSI oficial não foi obtido como binário válido neste ambiente; por isso foi exigida assinatura/publisher em runtime, sem inventar um checksum.
6. **Escopo de segurança:** não houve detonação de instaladores Windows, auditoria de cada pacote externo, prova formal de ausência de exfiltração ou avaliação completa de serviços de terceiros. O resultado é uma revisão com correções e evidência reproduzível dos riscos identificados.

## Compilar e usar esta revisão

O ZIP é uma distribuição de **código-fonte**, com o projeto existente completo, patch, lockfiles e evidências. Para obter os controles desta revisão, compilar esta árvore no sistema de destino. Downloads upstream, Homebrew e o pacote público `npx pandacode` não incorporam automaticamente estas alterações.

Instalar Node 22+, Rust estável 1.88+ e os [pré-requisitos oficiais do Tauri](https://v2.tauri.app/start/prerequisites/) para o sistema. No Windows, seguir a seção Windows para MSVC e WebView2. Esses downloads são etapas manuais de preparação do build.

```sh
cd pandacode-0.3.28-hardened
npm ci --ignore-scripts
npm run build
npm test
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
npm run tauri -- build -- --locked
```

Para desenvolvimento local, `npm run desktop`. O identificador do aplicativo foi preservado para compatibilidade: um build local pode abrir/migrar os dados de uma instalação anterior com esse identificador. A migração Jev foi projetada para preservar segredos em caso de falha. A interface About identifica `hardened.1`.

Reproduzir a verificação independente do backport/ZIP:

```sh
python src-tauri/vendor/glib/verify-backport.py
cargo test --release --locked --manifest-path src-tauri/security-tests/Cargo.toml
```

Atualizações futuras devem ser uma operação manual deliberada, com revisão do diff e manutenção destes controles. As fontes de website, documentação histórica e automações upstream foram preservadas para manter o projeto; não representam um instalador desta revisão.

## Fontes primárias consultadas

- [Tauri: GHSA-w28w-mhc8-qvjv](https://github.com/tauri-apps/tauri/security/advisories/GHSA-w28w-mhc8-qvjv).
- [DOMPurify: GHSA-mrv2-3pqr-j5jr](https://github.com/cure53/DOMPurify/security/advisories/GHSA-mrv2-3pqr-j5jr) e [GHSA-cg2g-xh64-v4q7](https://github.com/cure53/DOMPurify/security/advisories/GHSA-cg2g-xh64-v4q7).
- [Vite: GHSA-rq7h-c2jc-7f22](https://github.com/vitejs/vite/security/advisories/GHSA-rq7h-c2jc-7f22); demais IDs no JSON npm original.
- [KaTeX: GHSA-238p-pmpm-9mq7](https://github.com/KaTeX/KaTeX/security/advisories/GHSA-238p-pmpm-9mq7).
- [source-map-js 1.2.2](https://github.com/7rulnik/source-map-js/releases/tag/v1.2.2).
- [RustSec 0381](https://rustsec.org/advisories/RUSTSEC-2026-0381.html), [0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html), [0383](https://rustsec.org/advisories/RUSTSEC-2026-0383.html) e [ZIP 0367](https://rustsec.org/advisories/RUSTSEC-2026-0367.html). O JSON OSV inclui os demais avisos e referências.
- [Patch oficial GLib](https://github.com/gtk-rs/gtk-rs-core/pull/1343), commit `b5a4071e439bef2b5eea76c3aa25e5ae84839e34`.
- [Claude: variáveis e precedência](https://code.claude.com/docs/en/env-vars), [Kimi: variáveis](https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/env-vars), [OpenCode: CLI](https://opencode.ai/docs/cli/).
- [AWS: endpoints](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-endpoints.html) e [opções abreviadas](https://docs.aws.amazon.com/cli/latest/userguide/cli-chap-troubleshooting.html).
- [Vercel: opções globais](https://vercel.com/docs/cli/global-options), [Wrangler: comandos gerais](https://developers.cloudflare.com/workers/wrangler/commands/general/).
- [TightVNC: distribuição oficial](https://www.tightvnc.com/download/), [orientação do mantenedor sobre assinatura](https://sourceforge.net/p/vnc-tight/mailman/vnc-tight-list/?limit=25&page=7), [Microsoft: Get-AuthenticodeSignature](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/get-authenticodesignature).

O fixture de VM simula macOS dentro do navegador Linux; isso não equivale a testar Tart em um Mac real. A execução inicial e a repetição dos cinco casos estão preservadas nos logs. Warnings de estilo/lifetime do GLib, a variante de preços Live não utilizada e tamanho de chunks do Vite não foram ocultados; não impediram compilação/testes.

As evidências locais do comportamento do projeto são os arquivos originais identificados pelo hash, o patch e os testes incluídos. Links externos documentam advisories e contratos de fornecedores; não substituem essas evidências locais.
