> Nota de trabalho/evidência. O resultado consolidado de testes está em ../SECURITY-HARDENING.md e verification.json.

> Revisão independente anterior ao reforço final: o código final também recusa ledger existente truncado ou ausente. Ver notes-remote.md e os testes persistentes finais.

# Revisão independente final — replay persistente Connect

Revisão read-only de `src-tauri/src/remote/replay.rs`, inicialização em `remote.rs` e admissão em `remote/host.rs` e `remote/socket.rs`. Nenhuma alteração nesses arquivos, nenhum build ou cargo iniciado por este subagente.

## Resultado

Não foi identificado bypass de replay ou regressão bloqueante na implementação examinada.

- `Replay::check` valida timestamp e formato/limite do nonce, abre transação SQLite IMMEDIATE, verifica duplicidade por SHA-256 do nonce e primary key, insere recibo e confirma a transação antes de devolver sucesso. O host só executa depois desse sucesso; socket só entrega respostas/eventos após admissão. IMMEDIATE também serializa duas conexões/processos com o mesmo banco.
- Falha de abrir ou escrever SQLite permanece fail closed. `Replay::default` não é um cache efêmero: tem `db=None` e rejeita mensagens. Em erro de inserção/commit não há caminho que execute uma ação enquanto a API indica erro.
- A janela permite diferença absoluta de até 60.000 ms. A remoção usa `ts < now-60000`, portanto o recibo no limite inclusivo ainda é conservado. O limite de 20.000 recibos recusa mensagens novas sem expulsar entradas ainda válidas. Também existe limite SQLite de 2.048 páginas.
- Watermark `last_ms` e pruning são confirmados na mesma transação. Se o relógio recua, o gate bloqueia em vez de tornar novamente válido um ciphertext cujo recibo já foi removido. Transação falha não avança watermark nem confirma pruning parcial.
- `host::handle` checa confiança da chave/remetente e host habilitado antes de consumir nonce. `socket::admit_delivery` exige chave atual confiável para res/event; requests não são consumidos ali e são admitidos no host antes do dispatch. Timestamp e nonce de res/event são incluídos no payload autenticado antes de selar.
- Os testes presentes cobrem restart/segunda conexão, banco indisponível/read-only, rollback de pruning e watermark em falha de insert, capacidade/limite temporal e recuo de relógio após restart. Resultados de execução pertencem ao build central.

## Limites materiais

1. A garantia pressupõe continuidade do armazenamento privado. Excluir, truncar ou restaurar uma cópia antiga do SQLite pode recriar um histórico vazio/antigo; não é proteção contra rollback de disco por usuário/admin com acesso local. O relay remoto não obtém essa capacidade pela implementação revisada.
2. Uma grande correção do relógio para trás após uma admissão com horário adiantado bloqueia novas mensagens até alcançar o watermark. É escolha de segurança que pode afetar disponibilidade.
3. Se abrir o banco falhar no startup, o app mantém o gate indisponível nesta execução, imprime o erro e não tenta reabri-lo automaticamente. Reinício após resolver armazenamento/permissão é necessário. Respostas/eventos recebidos permanecem recusados; isso não significa bloqueio de toda conexão de rede/outbound do app.
4. Permissões 0700/0600 e NOFOLLOW protegem os caminhos normais contra exposição/acesso de outro usuário, mas não pretendem impedir usuário/admin com controle do diretório de dados e do processo. Windows usa ACL herdada do app data.

## Varredura final de credenciais hardcoded

Scanner `Secure Code Review/scripts/scan_secrets.py` utilizado como biblioteca para não emitir seu campo evidence, que apenas trunca o conteúdo. Saída limitada a caminhos, linhas, regras e classificações em `secret-scan-sanitized.json`.

Cobertura: 591 arquivos próprios, incluindo source/config/docs/scripts e extensões mjs/cjs/lock, excluindo node_modules/vendor, target/dist/build, fixtures esperados, arquivos de testes e módulos Rust `#[cfg(test)] mod tests`. Complemento verificou formatos modernos de tokens, JWTs, blocos PEM, URIs com credenciais e literais sensíveis com prefixo no nome da variável.

Foram revisados 127 alertas do scanner (100 linhas e 27 duplicações por entropia) e 2 alertas suplementares. Eram expressões dinâmicas, tipos, nomes de entradas no cofre, marcadores/labels e uma fixture de preview. **Nenhuma credencial real hardcoded foi confirmada neste recorte.** Não foram emitidos valores de possíveis segredos. Isso não substitui a análise dos fluxos de runtime e não comprova inexistência de todo segredo ou de material ofuscado.
