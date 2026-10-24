# PandaCode 0.3.34.1 hardened — auditoria direcionada e mitigação

**Revisão:** 2026-10-23 · **base de código:** ZIP 0.3.34 hardened (SHA-256 `7cf9a99bbfd3e3753d542753c954551007290e42e14464c43163b7d066f1a412`) · **alvo:** 0.3.34.1 hardened (SemVer `0.3.34-hardened.1`).

**Status global: `blocked-critical-evidence` — NÃO aprovado para produção.** Os patches e a inspeção de código não substituem `cargo test --locked`, builds Tauri, testes de integração, audit de dependências com dados atuais, isolamento de execução real, Windows/Keychain, deploy Cloudflare ou verificação independente. Foram adicionados testes de regressão de código-fonte, mas não executados. A análise é limitada aos oito achados e às superfícies próximas afetadas.

## Resumo por achado

| ID | Veredito sobre o relatório original | Alteração na 0.3.34.1 | Garantia / lacuna |
|---|---|---|---|
| F01 | **Suportado (alto, autorização cruzada)** | Associação obrigatória de confirmação a UUID de aprovação, uso único, TTL de 10 min e limite de fila. Resposta `yes` sem ID não aprova. | Código inspecionado; teste de A resolvido pelo desktop + resposta antiga versus B criado; teste não executado. O usuário precisa revisar a ação no desktop. |
| F02 | **Suportado (alto, risco de governança)** | Novas tarefas em storage/UI/GitHub/overlay usam `ask` por padrão; modo `full` só por escolha explícita. | Tarefas persistidas com `full` são preservadas, sem migração silenciosa. Verificar launchers/CLI em runtime. |
| F03 | **Suportado (médio, cancelamento)** | Processo externo de deploy supervisionado recebe flag atômica de parada, interrompe grupo/processo ao cancelar. Deadline de ferramentas de deploy ajustado para 1860 s versus limite interno de 1800 s. Resultado incerto explicitado. | Sem prova em Windows/Job Object; efeitos já enviados a APIs externas não podem ser revertidos; cancelamento externo de MCP exige validação adicional. |
| F04 | **Suportado (médio, timeout)** | Worker aplica `timeout --kill-after` ao comando, serializa execuções e destrói sandbox ao timeout confirmado ou ao fallback. | Destruir o container elimina estado não persistido no workspace; execução real e semântica Cloudflare não verificadas. |
| F05 | **Suportado (médio, supply chain)** | Removido `npm install` disparado pelo desktop; exige Wrangler previamente instalado/revisado e verificado na versão 4.151.0. | **Restrição funcional intencional**: sem dependências Worker previamente preparadas, deploy não está garantido. Não há lockfile auditado para o Worker; não declarar risco eliminado. |
| F06 | **Suportado (médio, supply chain)** | Substituído `npx wrangler` por invocador local sem fallback/download, versão 4.151.0 validada antes de comandos permitidos; npm ignora hooks. | Invocador requer Wrangler pré-provisionado. A dependência não foi inserida no lockfile do relay; funcionalidade falha fechada quando ausente. |
| F07 | **Suportado (médio, limites HTTP)** | Relay lê corpo via stream com limite real de 256 KiB antes de JSON/GitHub; 413 quando excede sem `Content-Length`. Worker /exec também limita bytes. | 3 testes de regressão definidos mas não executados em sandbox. Infra da plataforma não foi auditada. |
| F08 | **Suportado (baixo, governança)** | SECURITY.md diferencia distribuição hardened de upstream, proíbe atualização automática e indica procedimento manual. | Contato de segurança específico do fork não foi fornecido/verificado; não criar endereço fictício. |

Os achados F02, F05, F06 e F08 descrevem **políticas ou falhas de governança**, não backdoors, explorações observadas ou CVEs demonstradas. Não foram classificados como falsos positivos simplesmente porque não há reprodução dinâmica. F01, F03, F04 e F07 têm caminhos de código identificados e patches no alvo; a confirmação comportamental está pendente.

## Proveniência por entrada e saída (resumo)

- **F01:** `src-tauri/src/bots/gateway.rs` → `parse_command`, `approval_requested`, `answer_approval` → `respond_approval` em `src-tauri/src/lib.rs`. Ator: remetente previamente pareado, canal externo. Invariante: autorização específica e não reutilizável.
- **F02:** `src-tauri/src/storage.rs` + `src/main.ts`, `src/github.ts`, `src/overlay-client.ts`, `src/preview.ts` → `providers.rs`. Ator: conteúdo de agente; invariante: menor privilégio em tarefas novas.
- **F03:** `src-tauri/src/providers/host_tools.rs`, `src-tauri/src/providers/mcp_host.rs` → `src-tauri/src/cloud/deploy.rs` → `src-tauri/src/workspace.rs`. Ator: ferramenta do agente com permissão; invariante: nenhum efeito local indefinidamente após cancelamento. **Limite**: efeito remoto é indeterminado.
- **F04:** `src-tauri/src/cloud/cloudflare_worker/src/index.ts` → `container.exec` → `timeout` / `container.destroy`. Ator: detentor de token remoto; recurso: container do sandbox.
- **F05:** `src-tauri/src/cloud/cloudflare.rs` → Wrangler instalado no host, sem npm automático; falta de árvore de dependências pronta impede deploy.
- **F06:** `relay/package.json` → `relay/scripts/wrangler.mjs`, sem invocação `npx` ou download implícito.
- **F07:** `relay/src/index.ts` → `relay/src/limits.ts` → `githubUser`; corpo excedente rejeitado com 413 antes de solicitação GitHub.
- **F08:** `SECURITY.md` política de atualização e reporte do fork.

## Testes de regressão adicionados (NÃO EXECUTADOS)

- `src-tauri/src/bots/gateway.rs`: resposta atrasada A nunca aprova B; bloqueia `yes` sem UUID.
- `src-tauri/src/workspace.rs`: processo controlado ultrapassa deadline e é cancelado, sem criar arquivo marcador (Linux/Unix).
- `relay/test/request-body-limit.test.ts`: corpo de duas partes acima de 256 KiB sem `Content-Length`, corpo normal e JSON inválido.
- Testes existentes tiveram expectativas ajustadas para `ask` em novas tarefas.

## Validação, garantia e evidências pendentes

- Verificação estrutural de mudanças, bloqueio de `npm install` em `cloudflare.rs`, bloqueio de `npx wrangler` no relay, versão e lockfiles: **executada por script estático externo**, ver `static-verification.json`.
- Parser de sintaxe TS nos arquivos alterados (compilador TypeScript disponível no host, parsing sem executar o código): **executado**, ver `static-verification.json`.
- `node --check` sobre invocador JS: **executado**, ver `static-verification.json`.
- Testes de unidade/integrados, `cargo check --locked`, `cargo test --locked`, `npm ci`, `tsc --noEmit`, Playwright, Cloudflare real, Windows, macOS, SCA e varredura completa de segredos: **not-run** por ausência de isolamento/toolchain verificados, credenciais ou ambiente apropriado; não contam como pass.
- Resultado de deploy ou ferramenta abortado pode permanecer **indeterminado**; não fazer retry automático sem reconciliação externa.
- F04 destrói o container após timeout; dados não persistidos podem ser perdidos.
- F05/F06 falham fechados quando Wrangler/dependências não estão previamente disponíveis; instalação manual e validação independentes são condições para operação.
- Nenhum certificado, aprovação ou ausência de CVEs/segredos é reivindicado.

## Critérios para desbloquear uma release

1. `cargo check --locked` + `cargo test --locked` e testes de integração em Linux/Windows e cenários de aprovação atrasada.
2. Provar propagação do cancelamento em deploys e MCP, inclusive filhos e Windows Job Objects, e definir o tratamento de resultados remotos indeterminados.
3. Recriar worker Cloudflare com dependências fixadas/auditadas e executar teste de timeout observando a árvore de processos e efeitos posteriores.
4. Resolver supply chain do Wrangler/Worker com lockfile e distribuição reprodutível, mantendo scripts desabilitados por padrão.
5. Executar teste HTTP 413 sem Content-Length na implantação real e conferir ausência de chamada GitHub.
6. Rodar SCA contemporânea e secret scanning, revisar o suporte a Windows/macOS/instalador/antivírus.
7. Verificação independente que vincule ZIP final, árvore, toolchain e registros de execução.

**Resultado:** correções estáticas candidatas; liberação bloqueada por evidências críticas ausentes, inclusive funcionamento e eventuais regressões.
