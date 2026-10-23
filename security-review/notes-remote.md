> Nota de trabalho/evidência. O resultado consolidado de testes está em ../SECURITY-HARDENING.md e verification.json.

# PandaCode 0.3.28 — revisão de acesso remoto e canais

## Escopo e base

Fonte original: `audit/original/pandacode-0.3.28`, extraída do ZIP fornecido; SHA-256 do ZIP `4d74c67ab175967fa247852cd8d678e3908cb51de9d5a9292b1dd5dc6d500836`.
Candidato: `audit/pandacode-0.3.28-hardened`.
Arquivos revisados: `remote.rs`, `remote/*`, `relay/src/*`, `bots/channels/*`, `bots/gateway.rs` (interfaces de admissão/envio), `providers/mcp_host.rs`, `local_servers.rs`, configurações e documentação relacionadas.
Os números de linha abaixo referem-se ao original. Não houve execução do aplicativo original, detonação de amostras ou envio de conteúdo privado à rede. As provas isoladas executam exclusivamente funções puras extraídas dos arquivos.

## Achados corrigidos

### R-01 — Alta: autorização remota continuava válida após revogação

**Evidência:** `src-tauri/src/remote/socket.rs:205-237,292-297` armazenava `(mensagem, chave, trusted: bool)` na fila. `remote/host.rs:176-188` verificava esse booleano capturado, sem consultar novamente o pin atual. A fila sequencial pode aguardar outras chamadas. `remote/start.rs:163-215` faz descoberta de provedores, clone e worktree antes de iniciar o agente, sem nova consulta à autorização.

**Falha:** um dispositivo anteriormente confiável podia continuar executando solicitações enfileiradas depois que o usuário removesse sua confiança ou a chave mudasse. Um preparo demorado podia iniciar uma nova execução depois da revogação.

**Correção:** a fila carrega a chave pública que autenticou a mensagem. A execução consulta a lista e o pin atuais; preparação demorada consulta novamente antes das novas ações e antes do lançamento do agente. Revogação não desfaz ações que já tinham sido executadas.

**Teste:** `remote::tests::queued_messages_recheck_the_authenticated_key_after_revocation_or_replacement` cobre remover confiança, mudar chave listada, confiar em uma nova chave e remover dispositivo. Teste Rust completo deve constar da validação geral do projeto.

### R-02 — Alta: janela de troca de chave e entrega posterior à revogação

**Evidência:** `remote/socket.rs:402-405` chamava `callable()` e depois obtinha uma chave por `peer_key`, mas ignorava o segundo resultado `trusted`. A lista de presença podia mudar entre essas operações. `answer` enviava o resultado após uma chamada assíncrona usando a chave obtida anteriormente, sem confirmar confiança/hosting; eventos extraídos para envio também não reconfirmavam `host_enabled` no último ponto.

**Falha:** em uma condição de corrida, conteúdo de tarefas podia ser cifrado para uma chave substituída não confiável, ou continuar sendo enviado a um destinatário cuja confiança acabara de ser revogada. Exige Connect configurado e cenário de revogação ou controle/comprometimento do relay; não é acesso remoto anônimo por padrão.

**Correção:** a mesma chave pública usada na criptografia deve continuar listada e fixada no momento de inserir o frame na saída, com estado protegido pelo mutex. Respostas e eventos de host também exigem hosting ligado. Após revogação somente uma recusa genérica pode ser enviada à chave antiga, sem conteúdo de tarefa.

### R-03 — Média: overflow permitia furar a janela contra repetição

**Evidência:** `remote/host.rs:65` usava `(now_ms - ts).abs() > WINDOW_MS` em inteiros `i64`. Com `ts = now.wrapping_add(i64::MIN)`, a subtração/valor absoluto pode continuar negativo em compilação otimizada sem overflow checks; o teste temporal aceita esse valor. O registro antigo é elegível para descarte imediato na limpeza de nonces.

**Falha:** timestamp artificial de uma mensagem autenticada pode escapar da janela de ±60 segundos. Em modo com overflow checks ocorre panic. Isso não permite forjar a criptografia/autenticação de um dispositivo.

**Correção:** diferença absoluta não sinalizada por `abs_diff`, mais subtração saturada na retenção. A implementação final fica em `remote/replay.rs` e persiste a admissão antes de despachar, conforme R-08.

**Prova executada:** função original extraída e compilada com `rustc --test -C opt-level=2 -C overflow-checks=off` falha o teste que exige rejeitar o timestamp; implementação corrigida passa. Também há prova em compilação debug. Evidência: `audit/remote-regression/original-release-tests.log` e `hardened-release-tests.log`.

### R-04 — Média: desligar canal ou invalidar link não interrompia todas as entregas pendentes

**Evidência:** `bots/channels/mod.rs:330-340` enviava mensagens sem verificar `channel.enabled`. `bots/channels/web.rs:134-165` autorizava uma vez e aguardava até 25 segundos antes de retornar eventos, sem consultar novamente token, habilitação e permissão de acesso pelo túnel.

**Falha:** resposta de agente que terminasse depois do desligamento continuava indo ao Telegram/Slack/Discord/WhatsApp. Um poll autenticado antes de renovar o link podia receber novas respostas após a revogação.

**Correção:** consultar habilitação antes de cada parte de envio; o poll revalida token, canal e acesso remoto antes de cada leitura/entrega. A admissão do gateway é tratada separadamente pelo agente responsável por segredos/egresso.

### R-05 — Média: webhook de WhatsApp sem proteção contra repetição nem vínculo ao número

**Evidência:** `bots/channels/whatsapp.rs:26-45,67-75` verificava HMAC do corpo, mas descartava `message.id`, `timestamp` e `metadata.phone_number_id`. Toda entrega válida iniciava trabalho novamente.

**Falha:** repetição de um webhook assinado, inclusive reenvio do provedor, podia repetir comandos ou aprovações. Se uma aplicação Meta servisse vários números com o mesmo segredo, o corpo de um número podia ser aceito pelo canal de outro. Exige um canal WhatsApp explicitamente configurado e mensagem assinada; não é uma ausência de autenticação pública.

**Correção:** conferir número do canal; exigir ID e timestamp válidos; persistir recibo do ID/canal no `bot_activity` existente antes de despachar. A consulta e gravação usam o mutex de Store. Recibos sobrevivem à reabertura do banco. Timestamp mais antigo que 24h ou mais de 5min no futuro é recusado. O marcador de reset invalida mensagens anteriores caso o usuário tenha apagado o histórico do bot. Se houver 4000 ou mais eventos recentes, a admissão falha de forma fechada antes do limite de retenção de 5000 registros poder apagar a evidência. Não foi criada migração de schema.

**Tradeoffs:** comandos muito atrasados são ignorados; alto volume pode receber HTTP 503 para tentar depois. A garantia é admissão no máximo uma vez, não execução transacional exatamente uma vez: uma queda entre persistir o recibo e iniciar o trabalho pode perder uma entrega. Nenhum efeito já executado é desfeito.

**Testes adicionados:** banco reaberto e ID duplicado; idade/futuro/reset; capacidade; identificação do número, ausência de ID e overflow de timestamp. O teste preexistente de HMAC/payload foi atualizado com metadados válidos.

### R-06 — Média/robustez: limites de rede aplicados tarde e conexões de bot sem teto

**Evidência:** `remote/auth.rs:91-97` chamava `.bytes()` antes de conferir 1 MiB; o socket usava limites padrão da biblioteca em vez do teto de 256 KiB do protocolo. `bots/channels/http.rs:182-199` criava uma tarefa de até 40s por conexão sem limite de simultaneidade.

**Falha:** endpoint comprometido/resposta sem Content-Length podia consumir memória antes da recusa; muitos clientes locais ou por um túnel configurado podiam acumular tarefas. Não foi demonstrada execução de código por essas condições.

**Correção:** ler a resposta em partes e interromper antes de ultrapassar 1 MiB; limitar mensagem/frame WebSocket a 256 KiB; limitar a 64 conexões concorrentes no servidor de bots.

### R-07 — Baixa/defesa adicional: parser HTTP aceitava ambiguidades

**Evidência:** `bots/channels/http.rs:105-121` aceitava cabeçalhos repetidos com prevalência do último; não rejeitava `Transfer-Encoding`/`Expect`, versões e linhas iniciais ambíguas. Limite de cabeçalho podia ser ultrapassado no bloco que chegasse contendo o terminador.

**Correção:** recusar cabeçalhos duplicados, framing não suportado, linha inicial inválida, Content-Length não decimal e bytes de requisição adicionais; conferir tamanho antes de interpretar. Isso reduz divergências quando existe um túnel/proxy; a auditoria não demonstrou bypass isolado do bearer token por duplicação.

**Prova executada:** o teste de parser com Host/Authorization/Content-Length duplicados e caminhos inválidos falha no original e passa no corrigido, usando as funções reais extraídas sem dependências externas.

### R-08 — Média: eventos cifrados podiam ser repetidos pelo relay

**Evidência:** `remote/socket.rs:239-242` entregava responses/events autenticados diretamente ao cliente/mirror, sem timestamp ou nonce. Somente requests passavam por `Replay`. `remote/mirror.rs:112-162` atualiza o status do mirror com eventos e aceita `run.finished` para qualquer turno ativo da mesma sessão remota.

**Falha:** um relay comprometido podia reapresentar um ciphertext válido de `agent.events`/`run.finished` de um turno anterior, alterando estado visível ou encerrando prematuramente o turno local. A criptografia garante autenticidade, mas não impede repetição por si só. Respostas já dependem de IDs aleatórios de chamadas pendentes, o que restringe o impacto da repetição de respostas; a falha mais direta é nos eventos.

**Correção:** respostas e eventos recebem `ts` e nonce novo dentro do corpo cifrado no envio. A recepção verifica pin atual, janela de ±60s e nonce não recebido antes de despachar para o mirror. Mensagens legadas sem esses metadados são recusadas. Requests continuam sendo verificados na admissão do host, sem consumir o nonce duas vezes.

**Persistência adicional:** o cache original de requests também era perdido ao reiniciar o programa, permitindo reapresentar uma captura ainda dentro da janela temporal. A versão final usa `<diretório de dados>/connect-replay/receipts.sqlite`, separado do banco principal, para requests, responses e events. Armazena somente hash SHA-256 do nonce, timestamp e um marcador temporal. A transação SQLite `IMMEDIATE`, com `synchronous=EXTRA`, confirma o recibo antes da admissão e serializa duas instâncias no mesmo diretório. O diretório é 0700 e o arquivo 0600 em Unix; Windows usa a ACL herdada do diretório de dados do usuário. Não há fallback em memória nem recriação automática de banco corrompido. Arquivo truncado ou ausente em um diretório de recibos já existente também é recusado. Falha de abertura, gravação ou commit impede admissão. O limite é 20.000 recibos vivos, removendo apenas timestamps expirados; atingir capacidade recusa novas mensagens. O marcador temporal também impede que um ajuste do relógio para trás torne válida uma captura cujo recibo já tenha expirado e sido removido.

**Compatibilidade e limites operacionais:** os dois computadores precisam da versão corrigida; um peer 0.3.28 original não fornece a metadata exigida nas respostas/eventos. Se o relógio voltar para antes do último horário de admissão, Connect recusa novas mensagens até ele alcançar o marcador salvo. Esta correção não promete ordenação de eventos nem impede um relay de atrasar ou descartar uma primeira entrega. Para não depender de um relay, permanecer desconectado do Connect encerra essa superfície.

**Regressões adicionadas:** `remote::socket::tests::responses_and_events_need_fresh_unique_authenticated_metadata` verifica primeiro recebimento, repetição, ausência de metadata, idade, futuro, chave substituída e preservação da admissão dos requests. Quatro testes em `remote::replay::tests` verificam reabrir o banco/segunda conexão, dados gravados como hash, permissões Unix, storage indisponível ou sem escrita, rollback de transação após falha de inserção, capacidade/expiração e relógio retrocedendo após reinício. Os testes temporais anteriores usam a mesma implementação SQLite por um construtor em memória exclusivo de `cfg(test)`.

## Comportamentos opcionais que não são malware comprovado

- Connect é opt-in: `remote/config.rs` declara `host_enabled` falso por padrão. `remote.rs:235-253` abre o socket quando já existe login e configuração. Sem login não há esse tráfego. **Desligar Allow remote control não encerra a sessão no relay; Sign out é a ação que encerra o tráfego de Connect.**
- Em `remote/auth.rs:33,72-74`, login usa OAuth device flow com scope vazio e envia o token GitHub ao relay configurado para trocá-lo por token próprio. Isso é uma transferência explícita de credencial no fluxo de Connect, não um segredo embutido nem instrução escondida no prompt. Quem não quer confiar nesse serviço deve permanecer desconectado. A URL do relay vem de variáveis de build/ambiente; o arquivo-fonte sozinho não prova qual endpoint foi embutido num instalador publicado.
- A criptografia usa X25519/HKDF/XChaCha20-Poly1305, com pin por dispositivo; o relay do ZIP encaminha corpos cifrados e contém verificação do token de login contra o app OAuth configurado. Não foi localizada instrução oculta para exportar arquivos nesse subsistema.
- Canais de chat só funcionam após configuração e pareamento do remetente. Respostas comuns a uma conversa aberta por canal externo continuam saindo por esse canal, comportamento que precisa permanecer explícito ao usuário.
- MCP do agente escuta apenas IPv4 loopback em porta efêmera, exige Host literal e bearer por servidor, rejeita Origin e cabeçalhos ambíguos, impõe limites. `providers/mcp_host.rs:82-94,300-343`. Não foi identificado acesso anônimo ao host MCP nessas rotas.
- `local_servers.rs` enumera listeners locais e não os publica na rede; abrir a URL é ação explícita.

## Validação desta etapa

- 2 testes isolados contra código original: 2 falhas esperadas, mostrando parser permissivo e overflow.
- Os mesmos 2 testes isolados contra candidato: 2 aprovados.
- Reexecução do caso de timestamp em modo otimizado, com overflow checks desligados: original falha; candidato passa.
- Os testes isolados compilam trechos completos das funções de produção, sem reimplementar a lógica em outra linguagem; não substituem a compilação e a suíte Rust integrada.
- O módulo final `remote/replay.rs` foi incluído diretamente em um harness `rustc --test`, com as dependências Rust já compiladas para o projeto: **4 testes aprovados, 0 falhas**, incluindo as últimas verificações de ledger truncado/ausente. Não houve execução de cargo em paralelo. Logs: `audit/remote-regression/persistent-replay-compile.log` e `persistent-replay-tests.log`; o harness é `persistent-replay.rs`. Esse teste usa o módulo completo de produção, sem copiar/reimplementar sua lógica.
- Revisão independente por outro agente não encontrou defeito bloqueante de atomicidade, retenção, restart, clock ou uso da admissão no host/socket. Essa revisão foi somente por leitura. A proteção não constitui defesa contra um usuário/admin que restaure deliberadamente uma versão antiga de todo o armazenamento local; falha de abertura fica fechada naquela execução até reiniciar com o problema de armazenamento resolvido.
- A primeira suíte Rust integrada do fluxo principal (`audit/cargo-tests.log`) executou com êxito os sete testes novos de R-01/R-03/R-05/R-07 e o fixture WhatsApp atualizado; resultado global 1178 aprovados, 1 falha em expectativa de `file_preview`, 39 ignorados. Essa compilação antecede as regressões de R-08 e persistência, cujos resultados devem ser incluídos na validação final do fluxo principal. A etapa remota agora adiciona 12 testes ao total original.
- Regressões de UI, eventual reexecução da suíte e empacotamento pertencem ao fluxo principal. Registrar seu resultado efetivo antes de afirmar que passaram.
