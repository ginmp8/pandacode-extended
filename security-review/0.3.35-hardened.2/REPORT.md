# PandaCode 0.3.35-hardened.2 - implementação e verificação

Data: 24/10/2026. Artefato: codigo-fonte, não instalador nativo assinado.

## Resultado

Implementadas as melhorias locais de baixo impacto inspiradas nos padrões de uso do Paseo: perfis de conselheiro e comitê limitado; pré-verificação; inspetor de dependencias/bloqueios; central de atenção baseada no estado carregado; handoff com prévia, limites e confirmação. O executor original continua sendo o único responsavel por agentes e tarefas.

**Status da entrega:** implementação e verificações focadas aprovadas; **aprovação operacional nativa inconclusiva/bloqueada por evidências ausentes**. Nao ha afirmação de segurança integral, paridade completa com o Panda upstream, equivalencia com o Paseo ou ganho de CPU/RAM.

## Base exata e versão

- Entrada: `pandacode-hardened-feature-hardening.zip`.
- SHA-256: `3e436d75508f2ee91f12dbca160faa56ca32f812aada5449f65e40ed7748171f`.
- Versao declarada pela entrada: `0.3.35-hardened.1`; nova revisão: `0.3.35-hardened.2`.
- A referência a `0.35.0` não foi usada para renumerar a linhagem do produto.
- Os 1.060 arquivos originais foram preservados na base imutável. A linhagem histórica de backports seletivos permanece registrada em `HARDENED-REVISION.json`.

## Mudança mínima

Somente **um arquivo original de código**, `src/main.ts`, recebeu integração funcional. Foram adicionados quatro arquivos de runtime sob `src/hardened/`, totalizando 35.894 bytes de fonte, sem contar o pequeno adaptador em `main.ts`. Isso não e uma medição do bundle final, binário, memória ou CPU.

Demais arquivos originais alterados: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json` (somente versão); `HARDENED-REVISION.json` (linhagem/escopo); `README.md` (parágrafo da revisão). Foram acrescentados testes, ferramentas opcionais de desenvolvimento, documentacao, patch e evidências.

**Zero** novas dependências de runtime, serviços residentes, portas, comandos IPC, migrações de banco, caminhos de aprovação ou mecanismos de execução. Todos os arquivos Rust de implementação, os testes preexistentes, as capabilities, o CSP e os grafos de dependências permaneceram idênticos. A configuração do Tauri difere apenas na versão.

O patch funcional aplicou com `git apply --check` ao `src/main.ts` do ZIP original PandaCode 0.3.35. Esta evidência e textual; não equivale a build ou compatibilidade comportamental e não garante merges futuros.

## Funcionalidades e fronteiras

| Funcionalidade | Comportamento implementado | Limite deliberado |
|---|---|---|
| Conselheiro | Perfil com um revisor e critério explícito | Preparo restrito a Codex plan; envio manual |
| Comite | Dois revisores, duas atribuições, dois turnos do coordenador | Sem debate até convergir nem repetição ilimitada |
| Pre-verificacao | Relê tarefa, histórico e equipe; valida modelo, modo e orçamento | Nao substitui autorização nativa durante a execução |
| Inspetor | Dependencias, escopos, estados e motivos locais | Scheduler nativo e soberano; mostra limites de observacao |
| Atencao | Pendências, falhas e bloqueios carregados; navegação deduplicada | Nao aprova, responde, tenta novamente ou retoma |
| Handoff | Campos selecionados, checkpoints opcionais, prévia e cópia explícita | Redação de segredos e de melhor esforço; sem transcript completo |

Salvar um perfil usa a biblioteca existente sem aplicar a equipe ou mudar defaults. A seleção posterior no model picker conserva o comportamento preexistente de defaults do Panda. Inserir o prompt apenas preenche o composer vazio: não chama `send_message`, `architect_start`, `queue_add`, `architect_configure` ou comandos de aprovação. Anexos e rascunhos existentes provocam recusa, não substituição.

As consultas de refresh foram separadas de `reloadSessions`: o helper original também agenda retomadas automáticas. O novo painel não o chama. Fechar o painel remove os handlers e impede inserções tardias. Nomes, escopos e mensagens são renderizados como texto.

## Verificacoes executadas

| Verificacao | Base | Candidato | Interpretacao |
|---|---:|---:|---|
| Node completo, mesmos comandos/ambiente final | 134 aprovados / 4 falhas | 162 aprovados / 4 falhas | 28 novos casos aprovados; mesmas quatro pré-condições ausentes |
| Casos focados originais | 37/37 | Preservados | Sem alteração de asserts/fixtures originais |
| Browser do painel real com IPC sintetico | Nao existia | 15/15 | DOM, acoes, teclado, disposal, privacidade e layout |
| TypeScript completo, ferramenta 5.8.3 | 63 diagnósticos | Os mesmos 63, byte a byte | Dependencias ausentes; compilação completa não aprovada |
| Análise sintática de cinco módulos selecionados | - | Zero erros | Nao e typecheck completo nem build Vite |
| Scanner de segredos, tres novos TS e `main.ts` | - | Zero candidatos; JSON validado | Cobertura delimitada; não e auditoria integral/historico |
| Invariantes de diff e dependências | - | Aprovados | Backend, controles e testes originais preservados |
| Patch no original 0.3.35 | - | Aprovado textualmente | Sem execução do upstream |

As quatro falhas comuns são tres testes de bundle que precisam de `dist/index.html` e um arquivo de testes que não consegue importar `material-icon-theme`. Nenhum teste foi desativado ou relaxado para obter esses resultados. Nao foram instaladas dependências para ocultar os bloqueios.

O TypeScript disponível e 5.8.3, enquanto o projeto declara ^5.9.3. O resultado diferencial não substitui a verificação com a toolchain declarada. Nao foi executado build Vite, suite Playwright original completa, `test:release`, build Rust/Tauri, teste de providers reais nem benchmark. `cargo`/`rustc` não estavam disponíveis.

Os testes novos foram escritos e observados falhando antes da implementação correspondente. Regressões adicionais detectaram e corrigiram confirmação de handoff obsoleta, mudança do pedido durante espera assíncrona e exposição de bloco de chave truncado. A correção do fixture de navegação apenas omitiu um campo opcional `undefined` que o transporte Python convertia para `None`; nenhum critério de aprovação foi relaxado.

## Isolamento e significado da evidência

Os testes Node e o typecheck rodaram em namespace Linux com rede externa indisponível, apenas loopback isolado para testes MCP locais, ambiente limpo, fonte e toolchain somente leitura, escrita em área temporária e limites de CPU, memória, processos, arquivos e tempo. `containment.json` registra probes executados. Nao foram acessadas credenciais reais nem APIs de modelos.

O browser executou os novos módulos reais em HTML em memória, com requests de rede abortados e IPC substituido na fronteira. Chromium precisou de `--no-sandbox` no container descartavel; isso está explicitado na ferramenta de teste e não altera o produto. Esses cenários não provam o comportamento do WebView/Tauri, um provider real ou o isolamento de processos nativos.

## Revisao de segurança delimitada

- Nao foi adicionado transporte de codigo/credenciais, plugin remoto ou permissão de IPC.
- IDs de workers são verificados contra a tarefa pai antes de navegar; não existe novo handler de aprovação.
- O preflight exige Codex plan e um contexto novo. `readOnly: true` no assignment continua orientativo; a fronteira real depende do provider/native executor existente. A efetividade não foi demonstrada com tentativas reais de escrita nesta sessão.
- Handoff limita campos/tamanho, não coleta credenciais e exige confirmação do texto atual. A redação não cobre todos os formatos de segredo; o clipboard pode ser persistido por software externo.
- Nao houve auditoria atual de CVEs, auditoria integral de segredos, revisão independente de segurança ou prova de todas as plataformas. Relatórios anteriores permanecem históricos.

**Gate de assurance:** `blocked-critical-evidence` para aprovação operacional nativa, por ausência de build e validacao das fronteiras reais. Isso descreve uma lacuna de evidência, não uma vulnerabilidade confirmada.

## Reproduzir e manter

O guia `docs/HARDENED-REVIEW-TOOLS.md` documenta uso, comandos locais, ferramentas opcionais e rollback sem apagar dados. O patch em `patches/paseo-review-tools.patch` isola a extensao funcional. `source-manifest.json` fixa os bytes entregues fora desta pasta de evidências; `delta-receipt.json` identifica a base e as mudanças. O checksum externo identifica o ZIP inteiro.

Em um ambiente com as dependências e toolchains declaradas, ainda devem passar build frontend, testes originais, testes nativos e cenários reais de conselheiro/comite, inclusive negação de escrita. Nao usar apenas um merge textual limpo como critério de liberação.
