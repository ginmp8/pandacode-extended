# PandaCode Hardened 0.3.41 — implementação e validação

Data: **24/10/2026 (UTC)**. Produto: **`0.3.41-hardened`**. Distribuição: código-fonte completo.

## 1. Resultado e origem

Esta revisão parte do pacote fornecido **PandaCode Hardened 0.3.35.4** e incorpora seletivamente as melhorias priorizadas da referência **PandaCode 0.3.41**. A origem histórica do código permanece upstream **0.3.28**, com incorporações posteriores. O número do produto não representa substituição integral da base nem paridade completa com o upstream.

Os critérios foram congelados antes da implementação em [ACCEPTANCE.md](ACCEPTANCE.md). O trabalho foi dividido entre conexões/autorização remota, Skills e histórico/anexos, em worktrees independentes. A integração preserva o executor, o armazenamento seguro de credenciais, o banco e os componentes existentes. Os 1.139 arquivos do ZIP de origem continuam presentes.

| Entrada | SHA-256 |
|---|---|
| `pandacode-hardened-0.3.35.4(1).zip` | `479d00145a6c1794f4ca2aedfa2c1de7c5a84e4930c772fe63d2e948da005aab` |
| `pandacode-0.3.41.zip` | `7b988fcec724d6114d3144534bdb0e9a2b9078af2e10810d3fc36c7423e35b98` |

A comparação usou os arquivos fornecidos, extraídos com verificação de caminhos e symlinks. Não foi inferida autenticidade por assinatura desses ZIPs. Os links públicos do GitHub não puderam ser consultados pelo serviço de navegação nesta sessão; a referência técnica foi o código upstream fornecido, identificado pelo hash acima.

## 2. Funcionalidades incorporadas

| Critério | Implementação | Evidência principal |
|---|---|---|
| Nova sessão ao trocar de conexão | Atribuição, edição e remoção invalidam resume, contexto e estado nativo dependente da conta; o histórico fica no banco e é recuperado no próximo turno normal. Mutação é serializada com admissão e recusa tarefas/consoles ativos. | `connections::tests`, `providers::warm::tests`, cenário de troca explícita ao atingir limite. |
| Full access remoto independente | Configuração local com padrão `false`, inclusive em dados antigos. Verificação nativa de origem autenticada, modo e confiança em start/send/configure/steer/approval/input. | `remote::permissions`, `remote::host`, `remote::config` e `remote.spec.ts`. |
| Fingerprint remoto | Representação de 128 bits/32 dígitos hexadecimais. Chaves públicas e registros de confiança mantidos. | Testes de configuração/identidade e interface remota. |
| Catálogos GitHub de Skills | `gh` existente, host fixo, árvore em commit imutável, descoberta aninhada, imports explícitos, estado do catálogo e registro de origem/hash. | Testes de catálogo/importação e `skills-library.spec.ts`. |
| Skills por bot e projeto | Seleções por id/hash, união e origem, verificação durante admissão, contexto limitado, nenhuma escrita de conteúdo específico do bot no checkout compartilhado. | `skills::scoped`, `skills_admission_tests`, testes de UI. |
| Edit/Rewind | Fork exclusivo anterior à mensagem humana, nova tarefa sem resume, rascunho e snapshots restaurados; reserva nativa contra execução concorrente. | `conversation_fork::tests`, `history-upgrade.spec.ts`. |
| Anexos clicáveis no histórico | Chips individuais; arquivos por escopo nativo, reveal externo, imagens ligadas à mensagem/cache e links GitHub limitados. Formato antigo continua legível. | `message_attachments`, `prompt_attachments`, 12 testes Node e cenários de UI. |
| Limites por conexão | Claude/Codex leem o ambiente da conexão escolhida; caches por id/revisão, resposta vinculada à conta, estado explícito para casos não suportados. A continuação em outra conta exige clique. | Testes de leitura/cache e `hardened-041-connections.spec.ts`. |
| Seletor de agentes instalados | Oculta os não instalados que não são o atual; mantém o atual indisponível como desabilitado. Modelos continuam sujeitos ao catálogo existente. | Dois cenários dedicados e testes de conexões/modelos. |

O [guia em português](../../docs/HARDENED-0.3.41-GUIDE.pt-BR.md) identifica os caminhos exatos na interface. Documentação técnica complementar: [Skills](../../docs/skills-library.md), [conexões](../../docs/connection-session-safety.md), [Edit/Rewind](../../docs/rewind.md), [anexos](../../docs/message-attachments.md) e [consumo](../../docs/agent-usage.md).

## 3. Controles preservados e limites de desenho

### Conexões

A invalidação do contexto no SQLite termina antes de alterar configurações JSON ou segredos. Se uma gravação posterior falhar, pode ser necessário recuperar o contexto usando a conta anterior, mas o identificador nativo antigo não permanece utilizável com novas credenciais. Não existe uma transação distribuída entre banco, JSON e o cofre do sistema; essa ordem privilegia a separação entre contas.

O pré-aquecimento confere novamente conexão, revisão, sessão e geração antes de iniciar um processo. O console usa executável, argumentos e ambiente da conexão nomeada. Controles antigos do Codex que criam um cliente separado incapaz de respeitar a conta nomeada falham de forma explícita antes de iniciar esse cliente. A lista está em `docs/connection-session-safety.md`. Chat normal e os controles enviados ao processo já autenticado permanecem disponíveis.

A recuperação existente é limitada a 200 entradas e 64 KiB. Ela não transfere raciocínio privado, estado interno de ferramentas ou pixels de imagens. Edit/Fork cria tarefa sem atribuição automática da conta nomeada; o usuário pode selecionar a conexão antes de enviar.

### Autorização remota

O controle de Full access é adicional à confiança por dispositivo, não a substitui. Um dispositivo remoto não recebe um RPC para conceder a própria permissão. A configuração mantém os caminhos locais compatíveis e não altera automaticamente o modo de tarefas.

A confiança e a permissão são verificadas antes de admitir novas solicitações e antes de encaminhar novos comandos. **Revogar o dispositivo ou desabilitar Full access remoto não cancela retroativamente trabalho já admitido.** Um processo admitido pode ser criado posteriormente; comandos já aceitos no canal e continuações locais de um time já iniciado podem prosseguir. Para interromper esse trabalho, use Stop/Pause no anfitrião. Desligar apenas Full access ainda permite Stop e rejeição de aprovação de um dispositivo confiável.

O envio remoto para uma tarefa ocupada não é convertido em fila local anônima: a fila perderia a identidade de origem e permitiria execução posterior sem a verificação correspondente. A fila do visualizador e o steering explícito continuam disponíveis.

### Skills

Descoberta não executa conteúdo do repositório. Importação não ativa uma Skill. Commit, origem e hash são registrados; conteúdo local alterado, origem conflitante, symlinks, árvores truncadas e arquivos excessivos são recusados. Atualização de índice não substitui imports, ativações ou pins.

O fallback textual tem 8 KiB por item e 32 KiB agregados, incluindo cabeçalho. Ele entrega o texto principal, sem materializar automaticamente arquivos auxiliares. Uma ativação nativa saudável da mesma versão é reutilizada. Mudanças de seleção/versão/drift invalidam contexto antigo na próxima admissão; um turno em execução conserva seu snapshot.

Tarefas com fallback textual devem usar o chat; o console não é aceito como prova de que o prompt de Skills foi entregue. Ativações nativas saudáveis continuam compatíveis. A remoção explícita de uma Skill limpa somente pins cujos bots/projetos estão comprovadamente ausentes. Erros de consulta ou persistência abortam a remoção, sem tratar alvos indisponíveis como excluídos.

### Histórico e anexos

Edit não modifica a conversa original nem reverte arquivos/commits. O backend valida o papel humano da mensagem e a tarefa, e utiliza a reserva já existente para impedir corrida com execução. Forks de bots são tarefas independentes sem identidade de canal; recebem o brief atual e respeitam o estado pausado do bot.

O formato legado de anexos é textual e não oferece proveniência criptográfica. A UI e o backend respeitam fences: exemplos de `Attached image`, caminho ou `Bot brief` dentro de documentos/código permanecem texto. UUIDs de imagem exigem a estrutura esperada do registro, mensagem/tarefa vinculadas e arquivo PNG limitado. O reveal externo não executa o arquivo. Arquivos do projeto passam pelo resolvedor existente. Não foi alegada eliminação da janela TOCTOU herdada entre validação e abertura de caminhos locais.

### Consumo e compatibilidade

Limites de assinatura suportados são separados por conexão. **Custos monetários históricos continuam agregados por provedor**. Executáveis/argumentos personalizados, agentes sem leitor verificado e credenciais de API não recebem cotas de outra conta como substituto. Uma conexão alternativa do banner é compatível com o agente, mas não há garantia de cota restante.

Não houve migração SQLite, upgrade de dependências, novo provedor, telemetry, updater ou instalação automática. Comparações dos lockfiles confirmaram que somente a versão do próprio aplicativo mudou. Licença e atribuição foram preservadas. Isolated review permanece indisponível com `containment_unproven`, como na base.

## 4. Validação executada

Ambiente: Linux x86_64, Node **24.22.1**, npm **11.9.0**, Rust/Cargo **1.99.1** e Chrome for Testing **153.0.8010.12**. As dependências foram instaladas explicitamente no ambiente de validação; isso não introduz instalação automática no produto. `npm ci` utilizou `--ignore-scripts`; Cargo utilizou `--locked` e, após provisionamento, `--offline`.

| Gate | Base 0.3.35.4 | Candidato |
|---|---|---|
| TypeScript + Vite (`npm run build`) | Aprovado | Aprovado |
| Node (`npm test`) | 198 aprovados, 0 falhas | 210 aprovados, 0 falhas |
| Release/instalação (`npm run test:release`) | 40 aprovados, 0 falhas | 40 aprovados, 0 falhas |
| Rust nativo (`cargo test … --lib`) | 1.204 aprovados, 0 falhas, 39 ignorados | 1.254 aprovados, 0 falhas, 39 ignorados |
| Interface em Chrome | Comparações focais para falhas herdadas | 117 aprovados / 1 falha intermitente de preview WhatsApp na rodada JSON; 0 ignorados |
| Binário Linux sem empacotamento | Não executado | Compilação aprovada em perfil debug; executável não iniciado |
| Lockfiles, licença, arquivos da base | Referência imutável | Dependências/licença preservadas; todos os arquivos originais presentes |

Os 39 casos Rust ignorados continuaram ignorados; não foram contados como aprovados. Os testes de navegador usam preview/IPC de fixture e comprovam a interface, não autenticação ou execução com contas reais. A suíte Rust exerce o código nativo e políticas sem iniciar provedores pagos.

Os logs preservam a rodada inicial de interface e a rodada aceita, além dos resultados comparativos da base. A primeira rodada integrada teve 94 aprovações e 24 falhas: 21 expectativas/seletores herdados, uma fixture incompatível com o filtro solicitado de agentes instalados, o defeito real de Copy e o timeout do menu sob carga. A rodada seguinte encerrou com status aprovado, mas seu log de texto ficou incompleto; a contagem definitiva foi registrada em nova execução com reporter JSON, sem mudar fonte, seleção, asserts ou timeouts. Essa rodada aprovou 117 de 118 cenários; o único caso que falhou foi o wizard de WhatsApp ao esperar o callback após Save. O mesmo caso havia passado na rodada anterior com a mesma fonte e passou novamente em uma repetição isolada, sem mudanças de código, asserts ou timeout. O trace relaciona a perda do valor recém-preenchido ao redesenho durante o pareamento simulado de 400 ms; a validação HTTPS continuou recusando o valor HTTP anterior. Esta corrida específica não foi reproduzida na base nem testada no backend nativo. Esse resultado é tratado como instabilidade observada, não como uma suíte integralmente verde; todos os cenários novos das funcionalidades solicitadas passaram. A primeira tentativa Node do candidato ocorreu antes de gerar `dist` e falhou em três precondições de bundle; após o build, a suíte completa passou. Isso não foi tratado como uma alteração de código para fazer testes passarem.

### Achados e reparos durante o ciclo

- A revisão independente encontrou promoção indevida de marcadores dentro de documentos para anexos ativos e perda de texto no Edit. Scanners equivalentes em TypeScript/Rust e regressões eliminaram a reprodução concreta.
- A revisão de retomada de Skills encontrou a possibilidade de usar fingerprint/resume como prova de entrega ao console. O console passou a exigir o chat para fallback textual.
- Pins de alvos excluídos poderiam bloquear remoção para sempre. A limpeza conservadora foi integrada à remoção explícita, com cobertura de erros e alvos válidos inacessíveis.
- O pré-aquecimento e os controles nativos de Codex foram examinados para impedir uso silencioso de credenciais globais em tarefas nomeadas.
- A rodada de interface expôs seletores herdados desatualizados de bots e provider settings. Os testes foram alinhados aos controles reais e ao padrão Hardened `ask`, preservando as verificações de conteúdo, drafts e restrições. O teste de catálogo completo passou a declarar agentes instalados na fixture, de acordo com o novo critério de visibilidade.
- Um defeito de sobreposição do botão Copy durante streaming foi reproduzido também na base. O ajuste de espaçamento é localizado; o teste conserva o clique real, sem `force` ou relaxamento do oráculo.
- O primeiro teste do menu de anexos excedeu o prazo de abertura lazy durante compilação paralela. O trace mostrou os cinco itens corretos logo após o limite; a repetição isolada manteve o mesmo timeout e os mesmos asserts.

Não houve uso de conta pessoal, envio de credenciais a scanners externos ou inferência paga. O scanner obrigatório de fixtures de segredos e `git diff --check` fazem parte da conferência final. Avisos antigos de lifetime do GLib vendorizado e de tamanho de chunks do Vite permaneceram visíveis; não foram silenciados nem tratados como prova de falha de segurança.

## 5. Reproduzir

Com as dependências nativas adequadas ao sistema e um Chrome de teste instalado:

```sh
npm ci --ignore-scripts
npm run build
npm test
npm run test:release
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib -- --test-threads=4
cargo build --manifest-path src-tauri/Cargo.toml --locked --bin pandacode
PANDACODE_TEST_CHROME=/caminho/do/chrome PW_WORKERS=4 npx playwright test \
  tests/hardened-041-connections.spec.ts tests/history-upgrade.spec.ts \
  tests/skills-library.spec.ts tests/remote.spec.ts tests/connections.spec.ts \
  tests/message-actions.spec.ts tests/attachments.spec.ts tests/token-saver.spec.ts \
  tests/usage-limit.spec.ts tests/agent-usage.spec.ts tests/bots.spec.ts \
  tests/bots-web.spec.ts tests/provider-settings.spec.ts
python3 security-review/hardened.3/check-slack-fixtures.py .
```

Não execute compilação pesada simultaneamente ao teste de UI se a máquina tiver poucos recursos. `--offline` pode ser usado no Cargo depois de preparar o cache, como nesta execução. O caminho de Chrome acima é um parâmetro do ambiente, não um arquivo distribuído.

## 6. Fora da validação e da seleção

Não foram produzidos/validados instaladores Windows ou macOS, assinatura de distribuição, comportamento de Explorer/Finder nessas plataformas, execução gráfica da janela Tauri, acesso real ao cofre do usuário, provedores reais, GitHub autenticado ou dispositivos remotos reais. A compilação Linux não comprova esses ambientes. O Rust mínimo anunciado pela base não foi testado separadamente com toolchain antigo.

O console interativo opcional do Cline não foi incorporado; o provedor existente foi mantido. Permanecem excluídos remoção de confiança por dispositivo, updater, instalação automática, nova infraestrutura móvel/remota, transporte de credenciais/checkout para nuvens, leitura automática fora do projeto em Plan, migração de segredos para arquivos e novos provedores Jev/OpenRouter/Kilo.

Este relatório registra escopo, evidência e limites de uma revisão funcional/de segurança localizada. Não constitui auditoria exaustiva, certificação de segurança ou garantia de compatibilidade com todas as versões externas. Relatórios anteriores incluídos no pacote são históricos e não substituem os resultados desta revisão.

## 7. Conteúdo verificável da entrega

`verification.json` consolida os resultados e limites. `input-archives.json` identifica os insumos. `source-diff.json` relaciona arquivos adicionados/modificados com hashes de antes/depois, excluindo o novo diretório de evidências. `changes.patch` contém a diferença de fonte/configuração/documentação/testes contra a base. `source-manifest.json` lista tamanho e SHA-256 de cada arquivo empacotado, exceto o próprio manifesto. Os logs textuais e o JSON do navegador ficam em `evidence/`. Capturas novas de navegador, traces, dependências instaladas, builds, cache e repositório Git não são distribuídos.
