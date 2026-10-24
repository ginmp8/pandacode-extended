# PandaCode 0.3.35 hardened — revisão de backport e segurança

**Data da revisão:** 2026-10-24  
**Identificação do produto:** `0.3.35-hardened.1` / pacote-fonte `pandacode-0.3.35-hardened.zip`  
**Estado da evidência:** `STATIC_BACKPORT_VERIFIED` para o backport; **`review_status=blocked-critical-evidence`** para liberação operacional, porque execução e autorização runtime não foram provadas.  
**Base:** `pandacode-hardened-0.3.34.1.zip` — SHA-256 `094d2e05af75c488c346205dacf08fae6e898ef04f9221d2701e701636819103`.  
**Comparação:** `pandacode-0.3.35.zip` — SHA-256 `fc668a66394a3390f78b44a2cd9187b027f383ad1492ddf9653dd00c3f2b1cdd`.

## Decisão de escopo

A 0.3.34.1 hardened já possui **menu nativo** com tarefas e consumo de agentes. A inovação pontual documentada no release upstream 0.3.35 é o **painel visual da bandeja/menu bar**. Adotado como backport limitado; **não há equivalência funcional geral com a 0.3.35 original**.

**Implementado no código-fonte:**

- Painel sob o ícone com tarefas `running`, `waiting` e concluídas (incluindo distinção entre sucesso e falha), vínculo a agente/projeto, acesso ao task, navegação por setas e Enter.
- Barra visual dos limites de plano com percentual, estado 80%/95%, horário de reset, informação de cache antigo e refresh somente mediante abertura ou ação do usuário.
- Webview `tray` dedicada com capability de evento mínima; IPC Rust possui lista explícita de sete comandos `tray_*`, separada dos comandos do workspace e do launcher. Navegação permitida somente dentro do aplicativo com `view=tray`.
- Salvaguardas adicionais na adaptação: máximo de 1.024 tarefas concluídas ainda não vistas, 32 provedores consultáveis no painel por rodada, uma rodada de consultas de uso por vez, rejeição de percentuais não finitos para CSS, limite/validação da altura do painel existente no upstream.
- Mock de prévia e teste Playwright do upstream para o painel, sem executar testes de navegação no sandbox atual.

**Não incluído, intencionalmente:**

- Agentes em nuvem (`cloud_agents`), setup de ferramentas em máquinas remotas, upload de código e tokens/credenciais e sincronização automática: mudanças invasivas nos limites de confiança, nova autorização/telemetria e testes OS/rede seriam necessários.
- Novos provedores cloud/VM, scripts de deploy, armazenamento em arquivo plano de segredos no macOS, permissões ampliadas e fluxos de instalação remota. Na versão hardened, chaves continuam no mecanismo protegido preexistente.
- Updater do app e dos agentes, permissões de processo/restart, instalador `npx` remoto. A política de atualização manual do hardened anterior permanece intocada.
- Mudanças de dependências não necessárias, scripts de release, locks do upstream, alterações amplas em bots, HTTP/relay, Cloudflare, Connect, arquivos do workspace ou ferramentas MCP.

**Importante:** O código herdado já traz bloqueios e compensações de segurança que não são revalidados apenas por permanecem byte a byte idênticos. Os registros anteriores (`security-review/0.3.34.1/`) continuam com suas ressalvas de execução pendente.

## Evidências realizadas

| Verificação | Resultado | Limite |
| --- | --- | --- |
| Inventário ZIP, trajetos, links e duplicatas | Aprovado: ambas entradas seguras para a extração controlada (1141 e 934 entradas) | Não atesta ausência de código malicioso |
| Baseline congelado com hash de ambos os ZIPs, 21 arquivos críticos e escopo fixo | Realizado; `frozen-evaluation.json` | Hash prova identidade, não comportamento |
| Regressão estática dos controles protegidos, comandos IPC, versões e lockfiles | Aprovado, `static-verification.json`; script reproduzível `verify-source.py` | Inspeção sintática/estrutural, sem runtime |
| Parser TypeScript sobre `src/tray-popover.ts`, `src/entry.ts`, `src/preview.ts`, `tests/tray-popover.spec.ts` | **4/4 sem erros sintáticos** | Parser, não typechecker completo |
| Verificação sintática `node --check` do instalador npx preservado | Aprovado | Não executa instalador |
| `tsc --noEmit` em baseline e candidato | **Bloqueado por dependências não instaladas**: baseline 62 diagnósticos, candidato 63. Único novo diagnóstico: módulo já declarado em `package.json`, `@tauri-apps/api/event`, não encontrado no ambiente. | Não é compilação aprovada nem reprovação específica do código novo |
| Controle negativo do verificador (alteração byte-a-byte em arquivo crítico de cópia descartável) | Reprovado como esperado: escopo e SHA-256; `tamper-probe.json` | Demonstra sensibilidade do checker, não segurança do runtime |
| `cargo check --locked`, `cargo test --locked` | Não executados: Cargo/Rust indisponíveis | Sintaxe/compilação Rust não verificadas |
| Node suite, Playwright, aplicativo Tauri | Não executados: dependências/browser indisponíveis e isolamento completo para código não confiável não verificado | UX real, eventos do tray e integração nativa não verificadas |
| SCA atualizada, instalação, assinatura, Windows/macOS/Linux | Não executados | Não certifica segurança do produto nem da distribuição |

A separação de namespace de rede e mount read-only para o diretório-fonte foi observada; a tentativa de garantir **toda a raiz de filesystem somente leitura** falhou no sandbox disponível. Assim, não executamos scripts controlados pelo projeto nem testes de aplicação com acesso potencial ao host. Isso não é um resultado de falha do produto; é limitação de validação.

## Revisão de risco e governança (SGR-2.0)

| Item | Classificação | Evidência estática | Risco e decisão |
| --- | --- | --- | --- |
| R01 — IPC do novo painel | `needs-verification` / alta / confiança média | `lib.rs` roteia `tray` antes de `main`; `capabilities/tray.json` só habilita eventos; `tray.rs` restringe navegação. | A superfície adicional parece limitada; testar em runtime que nenhuma chamada de workspace/CLI/arquivo pode ser disparada pela janela `tray`. Não afirmar isolamento comprovado até o teste. |
| R02 — Renderização de títulos/percentuais | `needs-verification` / média / confiança média | `tray-popover.ts` utiliza `escape` nos dados dinâmicos, URLs de marca locais e valores percentuais finitos. | Testar payloads HTML/atributos malformados e valores extremos em runtime, com CSP real. Não identificado sink direto sem escape nos novos campos. |
| R03 — Consultas de consumo | `needs-verification` / baixa / confiança alta no código | Uso de `query_usage` preexistente; acionado ao abrir painel ou apertar refresh; limitador de solicitações simultâneas. | Consultas podem depender da CLI/provider e não são garantia de ausência de tráfego; documentar e testar consentimento, caches e limites. |
| R04 — Agentes cloud upstream | `not-applicable` / informativa / confiança alta | Arquivos de transporte/sync/credenciais não foram trazidos; `mod cloud_agents` ausente. | Evitou-se um novo fluxo de transferência de código e credenciais; não confundir com auditoria de quaisquer integrações cloud já existentes. |
| R05 — Hardening histórico F01–F08 | `needs-verification` / alta (gate de validação) / confiança alta na ausência de teste | Hashes iguais nos principais limites: bots, MCP/deploy, Cloudflare, instalações, HTTP relay. | Sem regressão textual nos arquivos protegidos, mas as lacunas de teste/runtime da 0.3.34.1 permanecem abertas. |
| R06 — Supply chain/atualizadores | `governance-risk` / média / confiança alta na lacuna de evidência | Dependências e versões fixadas preservadas; sem novo updater/instalador habilitado. | Ausência de SCA contemporânea, binário assinado e testes do instalador impede autorização ampla para distribuição operacional. |

Não houve tentativa de autenticação com credenciais, execução de ferramentas de agentes, instalação de pacotes, acesso a provedores de nuvem, envio de arquivos para APIs ou testes invasivos. O teste estático de strings em **novos arquivos de código** não identificou padrões usuais de segredos. Isso **não** significa que a distribuição completa e o histórico estejam livres de segredos.

## Matriz de autoridade da nova webview

| Ação originada da webview `tray` | Escopo | Origem da autorização | Efeito/falha |
| --- | --- | --- | --- |
| `tray_snapshot` | Leitura de tarefas visíveis, nomes de projeto e percentuais cacheados | Janela local `tray` definida em capability dedicada e allowlist Rust | Erro de storage retorna falha, não executa tarefas |
| `tray_refresh_usage` | Consulta dos limites de até 32 CLIs/provider ids já configurados | Clique do usuário em Refresh ou abertura do painel | Sem bypass de instalação; possível consulta de conta existente deve ser validada em runtime |
| `tray_open`, `tray_show_app`, `tray_hide`, `tray_resize` | Navegação e apresentação locais | Cliques/teclado na janela limitada | Sem execução de CLI/workspace/deploy no handler |
| `tray_quit` | Encerramento local do processo | Clique em Quit da UI ou menu nativo | Sem rollback de tarefas remotas preexistentes, já limitação do produto |
| Comandos gerais (arquivos, secrets, agent tools, bots, MCP, deploy) | **Sem permissão para webview `tray`** | Enforced pelo roteamento por label no código fonte | A negação efetiva exige teste de IPC nativo |

O novo painel não recebe permissões de workspace em `capabilities/default.json` e não cria execução remota. A separação de comando em `lib.rs` e a consulta explícita de refresh devem ser verificadas no aplicativo real, não apenas por inspeção.

## Critérios ainda exigidos antes de uso operacional

1. Em ambiente isolado com Rust e dependências já provisionados, executar `cargo check --locked --offline` e `cargo test --locked --offline` no projeto `src-tauri/`; verificar `tray_position_centers_under_icon_and_clamps` e regressões de segurança históricas.
2. Com node_modules gerado a partir do lockfile sem scripts de instalação não aprovados, executar `./node_modules/.bin/tsc --noEmit`, a suíte Node e o teste `tests/tray-popover.spec.ts` usando o navegador local aprovado.
3. Testar as três plataformas: clique esquerdo/direito, abrir/fechar/foco da janela transparente, layout, teclado, conclusão/falha de tasks, refresh, URLs de navegação, erro de rede/provedor e fechamento da aplicação.
4. Tentar invocar comandos de alto privilégio (`run`, arquivos, bot, MCP, cloud, terminal) pela webview `tray` e comprovar negação; verificar que `tray_snapshot` retorna só os dados previstos e que os outros webviews não herdam as permissões do painel.
5. Verificar integrações/segredos e CVEs contra versões efetivamente fixadas, operações de cancelamento, HTTP/body, aprovação de bots e origem de pacotes; tratar riscos ainda abertos na 0.3.34.1.
6. Verificar o binário final, assinatura/reputação do instalador e integridade em máquinas-alvo. Após isso realizar revisão independente ligada aos hashes do ZIP e dos testes.

**Veredito:** backport-fonte entregue e **aprovado somente em critérios estáticos**; `review_status=blocked-critical-evidence` para execução/distribuição operacional pela ausência de evidência dinâmica. Não é certificado de ausência de vulnerabilidades ou de compatibilidade total com a versão upstream.

## Material de auditoria

- `changes.json` — listagem exata de arquivos alterados/adicionados; `changes.patch` — diff do backport com contexto, sem a árvore recursiva de evidências.
- `frozen-evaluation.json` — limites, entradas e hashes protegidos congelados antes da implementação.
- `verify-source.py` — verificador de invariantes textual e hash; `static-verification.json` — recibo da execução.
- `source-manifest.json` — SHA-256 de cada arquivo do ZIP, exceto o próprio manifesto.

Os arquivos desta revisão são documentação de uma candidata fonte. A presença de logs antigos dentro da árvore não transforma testes históricos em testes atuais.
