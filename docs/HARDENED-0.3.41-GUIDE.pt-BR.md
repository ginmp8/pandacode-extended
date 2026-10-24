# PandaCode Hardened 0.3.41 — guia de uso

Esta versão foi criada a partir do pacote completo **Hardened 0.3.35.4**. O número interno é `0.3.41-hardened`. A referência de funcionalidades é o upstream 0.3.41; a origem histórica do código continua sendo 0.3.28 com incorporações seletivas posteriores. O pacote contém código-fonte completo, sem instalador nativo pré-compilado.

## Onde encontrar cada melhoria

| Recurso | Caminho na interface | Resultado |
|---|---|---|
| Conexões e troca de conta | **Settings → Providers**; depois seletor de agente/modelo no composer | A troca preserva a conversa e inicia contexto nativo novo. |
| Autorização remota independente | **Settings → Remote access → Full access for remote tasks** | Desligada por padrão; somente o computador anfitrião pode habilitar. |
| Fingerprint de dispositivo | **Settings → Remote access**, lista de dispositivos | Exibição de 128 bits; confiança continua associada à chave pública real. |
| Catálogos GitHub | **Settings → Skills & rules → GitHub catalogs** | Descobrir, selecionar e importar Skills em commit fixo. |
| Skills por bot/projeto | **Settings → Skills & rules → Bot & project skills** | Escolher escopo, alvo e versões; clicar **Save skill selection**. |
| Skills efetivas da tarefa | Na mesma página, **Current task: effective skills** | Exibir origem, forma de entrega e motivo de itens ignorados. |
| Edit/Rewind | Ação **Edit** em uma mensagem anterior enviada pelo usuário | Criar nova tarefa com o histórico anterior e rascunho editável. |
| Anexos no histórico | Chips junto à mensagem do usuário | Abrir arquivo, revelar origem, ampliar imagem ou acessar issue/PR. |
| Limites por conexão | Ícone **Agent usage**, próximo de Help/Settings; aba do agente | Separar configuração padrão e cada conexão nomeada. |
| Agentes instalados | Seletor de agente/modelo no composer | Oferecer os instalados; conservar o atual indisponível como desabilitado. |

Os nomes acima correspondem aos rótulos em inglês do aplicativo. Não é necessário instalar outra interface para encontrar os recursos.

## 1. Trocar de conexão com histórico preservado

Cadastre as conexões em **Settings → Providers**. No seletor do composer, escolha o grupo da conexão e o modelo. A tarefa continua com o mesmo histórico visível, mas perde o identificador de retomada da conta anterior. A mensagem seguinte utiliza o mecanismo de recuperação de contexto já existente.

Editar ou remover uma conexão também invalida os contextos das tarefas vinculadas. Uma tarefa ou console em execução deve ser interrompido antes da alteração. Selecionar novamente a mesma conexão não provoca uma reinicialização desnecessária.

A recuperação transfere no máximo 200 entradas recentes e 64 KiB. Ela não transfere raciocínio privado, estado interno das ferramentas nem os pixels de imagens anteriores; reanexe imagens necessárias. A troca de conta não desfaz alterações de arquivos.

Alguns controles antigos do Codex abrem um cliente separado com a conta padrão. Nesta versão, esses caminhos recusam tarefas com conexão nomeada antes de abrir o cliente: consultas independentes de conta/configuração/importação, inspeção de goal salvo, determinadas preferências Plan/Fast, operações nativas de fork/adopt/archive/rename e abertura de conversa Codex salva no console quando exige essa inspeção. Chat normal, controles encaminhados ao processo já autenticado e Edit/Fork do PandaCode continuam disponíveis. Consulte a lista em [connection-session-safety.md](connection-session-safety.md).

## 2. Full access remoto

**Full access for remote tasks** começa desligado, inclusive quando configurações antigas são carregadas. Dispositivos confiáveis continuam precisando de autorização independente do anfitrião para iniciar, continuar ou alterar uma tarefa em Full access. Habilitar o controle não muda automaticamente o modo de nenhuma tarefa.

As verificações ocorrem no backend e são repetidas na admissão da execução. Confiança, revogação, chave pública, hospedagem e proteção contra replay continuam sendo verificadas. O dispositivo remoto não recebe uma operação para habilitar a própria permissão. Revogar um dispositivo ou desligar a autorização impede novas solicitações e admissões remotas; não cancela retroativamente turnos, comandos ou objetivos já aceitos. Para encerrá-los, utilize **Stop** ou **Pause** no anfitrião.

O envio remoto a uma tarefa ocupada não entra silenciosamente na fila local sem identidade de origem. Aguarde a liberação ou utilize a ação explícita de steering disponível. Essa restrição impede que um pedido continue executável depois de perder o contexto de autorização.

O fingerprint agora apresenta 32 caracteres hexadecimais, correspondentes a 128 bits do resumo da chave. Não houve troca de chaves, remoção de dispositivos confiáveis nem substituição da autenticação por comparação de texto.

## 3. Descobrir e importar Skills do GitHub

1. Abra **Settings → Skills & rules → GitHub catalogs**.
2. Informe `owner/repository` e, opcionalmente, branch, tag ou commit.
3. Clique **Add catalog**. A descoberta usa o `gh` já instalado e configurado para GitHub.com.
4. Revise os pacotes encontrados, inclusive em diretórios aninhados.
5. Selecione os itens e clique **Import selected**. **Select all** prepara a seleção em lote.
6. Revise os findings e ative pela biblioteca existente ou associe ao bot/projeto.

Descobrir e importar não executa scripts, inicia agente nem ativa automaticamente uma Skill. O registro mantém origem, caminho, ref, commit imutável e hash SHA-256 do conteúdo importado.

**Refresh catalog** atualiza somente o índice. **Update available** informa mudança de commit, sem substituir cópias ou seleções. **Disabled** e **Ignored** impedem atualização e importação. **Remove catalog** remove o índice e preserva as cópias já importadas.

Conflitos de nome/origem e alterações locais bloqueiam sobrescrita. Um lote pode importar alguns itens e ignorar outros; a interface informa cada motivo. Pacotes grandes, árvores truncadas, caminhos inseguros, symlinks e submódulos são rejeitados ou excluídos pelos limites descritos em [skills-library.md](skills-library.md).

## 4. Associar Skills a bots e projetos

Na seção **Bot & project skills**, escolha **Project** ou **Bot**, selecione o alvo, marque as Skills e clique **Save skill selection**. Se houver findings, revise-os na biblioteca e confirme a aceitação específica das versões selecionadas.

Cada seleção guarda id e hash. Uma nova importação não altera silenciosamente as versões aprovadas. Após uma atualização, revise e salve novamente a seleção. Itens removidos, modificados, incompatíveis entre bot e projeto ou ainda sem aprovação aparecem como ignorados, com motivo.

Uma tarefa combina os escopos de seu bot e projeto e deduplica itens compartilhados. A origem aparece como `bot`, `project`, `both` ou `user`. A entrega aparece como `native`, `prompt` ou `skipped`.

Seleções específicas adicionam texto limitado ao contexto, sem escrever no checkout compartilhado. O limite é 8 KiB por Skill e 32 KiB no total, incluindo cabeçalho. Uma ativação nativa existente, íntegra e da mesma versão é aproveitada sem duplicar texto. A mudança de Skills invalida a retomada antiga na próxima admissão e recupera o histórico sem o bloco sintético anterior. Um turno já iniciado conserva seu snapshot; a mudança vale para o próximo.

O fallback textual entrega o conteúdo principal de `SKILL.md`. Arquivos auxiliares não são materializados automaticamente para bots nem lidos fora do projeto. Skills que dependem desses arquivos podem exigir ativação nativa explícita no projeto. Tarefas com esse fallback devem usar o chat: o console interativo não consegue comprovar a entrega do texto e permanece bloqueado enquanto houver esse contexto. Com ativações nativas saudáveis, após alterar seleções envie uma mensagem normal antes de retomar o console.

## 5. Edit/Rewind e anexos

Em uma tarefa parada, clique **Edit** na mensagem do usuário a partir da qual deseja refazer a conversa. O PandaCode cria uma tarefa independente, copia apenas o histórico anterior à mensagem escolhida e carrega seu texto no composer. Revise o rascunho, os anexos e a conexão antes de enviar.

A conversa original permanece intacta. A tarefa nova não reutiliza o resume ID antigo e não herda automaticamente uma conexão nomeada; escolha a conta desejada no seletor. Forks independentes criados no projeto de um bot podem ser reabertos em **Bot settings → Activity → Conversations → Independent tasks**. **Editar não reverte o filesystem**: alterações produzidas por mensagens posteriores ainda podem existir nos arquivos.

Os chips mostram os anexos de cada mensagem. Arquivos do projeto utilizam as abas existentes; itens externos são revelados pelo gerenciador do sistema; imagens disponíveis abrem uma visualização ampliada; links GitHub válidos de issues e pull requests abrem no navegador. Clicar em conteúdo histórico não elimina as validações nativas de caminhos, mensagem de origem e permissões.

Se um arquivo mudou ou uma imagem não está mais disponível no cache, o rascunho informa a limitação. Revise o conteúdo restaurado antes do reenvio. Exemplos textuais dentro de blocos de código permanecem texto e não são promovidos automaticamente a anexos.

## 6. Limites por conta e escolha de agentes

Abra **Agent usage** e a aba de Claude Code ou Codex. Cada conexão nomeada mostra seu próprio resultado, separado de **Default setup**. Dados indisponíveis permanecem indisponíveis. Configurações com executável/argumentos personalizados e outros provedores sem leitor verificado não recebem números da conta padrão como substituto.

O painel separa **limites de assinatura por conexão**. A estimativa monetária histórica continua agregada por agente/provedor; não foi inventada uma distribuição de gastos por conta.

Quando uma tarefa atinge um limite informado, a faixa pode oferecer **Continue on…** para outra conexão do mesmo agente. A escolha é explícita, inicia uma sessão nativa nova e não garante que a conta alternativa tenha cota restante. Não há troca automática.

O seletor de modelos exibe agentes instalados. Se o agente da tarefa atual deixou de estar disponível, ele permanece visível e desabilitado, permitindo entender o estado sem oferecer uma execução impossível.

## 7. Compilar, validar e conhecer os limites

Para o frontend:

```text
npm ci --ignore-scripts
npm run build
npm test
npm run test:release
```

Com Rust e as dependências nativas do sistema instaladas:

```text
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib
```

No Windows, siga [install-windows-hardened.md](install-windows-hardened.md). Esta entrega não contém um instalador Windows/macOS assinado nem comprova funcionamento com suas contas reais. Resultados executados, comandos, comparação com a base e limitações estão no [relatório da versão](../security-review/0.3.41-hardened/REPORT.md).

O console interativo opcional do Cline não foi incorporado. O provedor Cline existente foi preservado. Também permanecem excluídos updater, instalações automáticas, novos provedores de nuvem, migração de segredos para arquivos e remoção da confiança por dispositivo.
