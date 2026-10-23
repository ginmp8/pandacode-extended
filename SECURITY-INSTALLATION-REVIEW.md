> **Relatório histórico da revisão hardened.2.** O ajuste de fixtures da versão hardened.3 está documentado em `security-review/hardened.3/README.md`.

# PandaCode 0.3.28 / hardened.2: revisão da instalação

**Resultado: lacunas de instalação e empacotamento corrigidas; causa do alerta Avast ainda não determinada.**

Data: 23 de outubro de 2026. Este trabalho modifica o projeto existente; não é uma reimplementação.
O pacote contém fontes, não um EXE/MSI compilado ou assinado. Não foi executado no Windows nem submetido a uma validação Avast.

## 1. Identidade, contexto e limites

Base efetivamente examinada: `pandacode-0.3.28-hardened.1.zip`, SHA-256:

```text
4100f1ec4ac1c1629fd1d4c01137071cc41d2436d5b2cea6f0fb222a69381b17
```

A extração foi validada contra caminhos de fuga, duplicidades e links simbólicos; a base foi preservada separadamente.
O usuário relatou que o alerta `IDP.HELU.PSE95 - Command line detection` apareceu durante a instalação.
A imagem identifica PowerShell, Behavior Shield e estado bloqueado, mas não mostra a linha completa, o processo pai ou o instalador executado.
O arquivo entregue anteriormente não contém EXE/MSI, DLL, PS1, BAT ou CMD. Portanto, não há identidade verificável do binário local que gerou o alerta.

Escopo desta rodada: configuração Tauri, instaladores/empacotamento, scripts de release, instalação npm, ponte npx, instruções de instalação e principais invocações PowerShell presentes nos fontes.
Não é uma nova auditoria exaustiva de todos os provedores, dependências, prompts ou integrações. Não houve verificador independente nesta rodada.
As constatações abaixo são comportamentos de fonte/configuração e resultados dos testes locais, não CVEs ou explorações confirmadas.
Não foi identificada, nos caminhos de instalação inspecionados, instrução oculta de exfiltração; isso não certifica ausência de malware.

## 2. Lacunas e correções

### INST-01: download/execução automática de WebView2

Na base, `scripts/release/build.mjs` configurava `downloadBootstrapper` com execução silenciosa. A configuração Tauri principal também não estabelecia uma política alternativa de Windows.
Assim, um instalador produzido por esses caminhos poderia buscar e executar o pré-requisito externo na máquina do usuário.
Isso é comportamento legítimo de instalação de um componente Microsoft, **não evidência de malware nem de que ele disparou o Avast**.

Correção: `webviewInstallMode: { type: "skip" }` no plano de release e na configuração principal; NSIS explicitamente `currentUser`.
O novo overlay `src-tauri/tauri.windows.conf.json` seleciona NSIS para o build direto Windows.
Arquivos: `scripts/release/build.mjs:41-59`, `src-tauri/tauri.conf.json:66-74` e `src-tauri/tauri.windows.conf.json`.
Efeito: o usuário deve providenciar WebView2 separadamente. Sem o runtime o aplicativo não funciona.
Não foram desativadas atualizações de segurança do Windows/Edge/WebView2 administradas fora do aplicativo.

### INST-02: infraestrutura residual de updater no build

O script de release da base podia ativar `createUpdaterArtifacts`, encaminhar chaves de assinatura herdadas, assinar/copiar artefatos de atualização; a CLI `updater-manifest.mjs` ainda gerava `latest.json`.
O plugin de atualização do aplicativo já tinha sido removido em hardened.1. **Gerar esses arquivos não prova que o aplicativo estivesse atualizando automaticamente em runtime.**

Correção: geração de artefatos sempre desativada, remoção dos passos de assinatura/cópia, filtragem das chaves herdadas de updater e recusa explícita da CLI de manifesto, mesmo com entradas válidas.
Referências: `scripts/release/build.mjs:19-59`, `scripts/release/updater-manifest.mjs:16-21`.
Helpers puros/offline e seus testes foram preservados; eles não publicam nada automaticamente.

### INST-03: scripts npm dependiam de uma opção digitada manualmente

Sem `.npmrc` no projeto, a proteção anterior dependia de lembrar `--ignore-scripts` a cada instalação.
Os hooks de instalação de dependências executam código com os direitos e ambiente do processo npm. Não foi demonstrado que uma dependência deste pacote fosse maliciosa.

Correção: `.npmrc` com `ignore-scripts=true`; instruções mantêm `npm ci --ignore-scripts`.
Um teste executou `npm install --offline` em fixture local com postinstall inofensivo que cria um arquivo-sentinela: a base executou o hook; a revisão não executou.
O teste limpa configuração npm herdada para comprovar a política do arquivo, em vez de um resultado mascarado pelo ambiente.
A opção não é um sandbox e pode ser substituída intencionalmente por configuração de maior precedência.
Comandos explicitamente solicitados por `npm run` continuam funcionando. Scripts Rust `build.rs` não são abrangidos.

### INST-04: resolução de dependências Rust sem trava no empacotamento

Os planos de release não passavam `--locked` ao Cargo. Havendo necessidade de resolução diferente, um build podia alterar o lockfile em vez de falhar.
Correção em `scripts/release/build.mjs:59` e scripts `package`/`package:fast` do `package.json`: encaminhar `--locked`.
Isso reforça a identidade da resolução; não bloqueia downloads nem executáveis de build e não prova que dependências são seguras.
Os lockfiles e as versões de dependências não foram alterados nesta rodada.

### INST-05: recomendação de instalador remoto canalizado para shell

A seção Grok da UI (`src/provider-settings.ts:71`) e `docs/providers.md` sugeriam baixar um script remoto e encaminhá-lo diretamente para bash.
Era orientação exibida, não execução automática nem prova de comportamento malicioso do fornecedor.
Substituída por instalação manual de versão revisada/fixada. A integração Grok continua disponível.

### INST-06: publicação upstream e confusão de artefatos

O workflow `.github/workflows/release.yml` publicava releases/feed e podia publicar o pacote npm quando configurado.
Ele não roda apenas por instalar o aplicativo no PC; era infraestrutura de repositório incompatível com esta distribuição local revisada.
Foi removido. `packages/npx/package.json` recebeu `private: true`; o instalador npx já bloqueado na revisão anterior continua bloqueado.

O build local agora distingue os resultados em `release/0.3.28/hardened.2/` com nome `PandaCode-hardened.2-...` e sidecar SHA-256.
README, CONTRIBUTING e guias não orientam obter esta revisão por clone/instalador upstream. Avisos de bypass de alertas foram retirados do guia de plataformas.
**Único arquivo de produto removido: `.github/workflows/release.yml`.** Os antigos resumos de evidência foram realocados, não descartados.

## 3. PowerShell preservado e alerta não atribuído

| Caminho | Uso constatado nos fontes | Relação demonstrada com o alerta |
| --- | --- | --- |
| `src-tauri/src/local_servers.rs:43-62` | Consulta local de portas em escuta e processos, por `Get-NetTCPConnection`/`Get-Process`; argumentos fixos e sem profile | Nenhuma |
| `src-tauri/src/terminal.rs:622-641` | Consulta local de PID, PID pai e nome, por `Get-CimInstance Win32_Process` | Nenhuma |
| Terminal interativo | Shell escolhido para comandos autorizados do usuário/agente | Nenhuma; comando executado não fornecido |
| `src-tauri/src/cloud/aws.rs:119-153` | Script de instalação TightVNC dentro de uma VM Windows AWS, sob provisionamento explícito, com verificação Authenticode herdada | Não é o instalador local do PC |

Nenhum desses usos foi renomeado, ofuscado ou substituído para fugir do antivírus. Nenhum arquivo Rust foi modificado nesta rodada.
O produto continua executando comandos e usando os provedores escolhidos, como solicitado.
Sem a linha completa bloqueada, processo pai e hash/origem do EXE usado, não é possível distinguir a causa do alerta entre o empacotamento, uma dependência/ferramenta externa, comandos do ambiente ou um falso positivo.
As correções são justificadas por caminhos verificáveis, **não por reprodução da detecção**.

## 4. Verificação efetivamente executada nesta rodada

| Verificação | Resultado |
| --- | --- |
| Onze novos testes de política contra hardened.1 | 11 falhas esperadas; o teste detecta as lacunas originais |
| Os mesmos testes contra hardened.2 | 11 passaram |
| Suíte existente de release/npx | 39 passaram; nenhuma falha |
| Suíte Node completa tentada | 134 casos TAP: 130 passaram, 4 falharam por entradas ausentes |
| Recorte explicitamente executável da suíte Node | 130 passaram; inclui os 11 testes novos, não somá-los novamente |
| Sintaxe/transpilação dos dois TypeScript alterados | Passou; não equivale ao typecheck completo |
| Sintaxe dos seis JavaScript alterados | Passou |
| Build completo TypeScript/Vite/Tauri/Rust | Não executado |
| Instalador/assinatura/execução Windows e verificação Avast | Não executados |

As quatro falhas da suíte completa foram três testes de bundle sem `dist/index.html` e a carga de `tests/file-icons.test.mjs` sem `material-icon-theme` instalado.
Os arquivos de teste permanecem no projeto; não foram apagados, enfraquecidos ou marcados como sucesso.
A seleção reduzida está identificada no log e no runner externo. O total de aprovações distintas entre as suítes disponíveis e release é 169; isso não significa cobertura completa.

Os testes existentes de política foram ajustados para a nova exigência explícita (`false`, em vez de campo ausente; sem bootstrap/updater), sem remover asserções de segurança alheias ao escopo.
As primeiras tentativas tiveram incompatibilidades do ambiente de teste (carregamento TypeScript, WebAssembly e loopback); os logs foram preservados, e a execução final usa um ambiente corrigido, não asserts reduzidos.

Execução isolada em Linux: namespaces de rede/PID/mount/user, chroot, capacidades removidas, ambiente allowlist sem credenciais, fonte/toolchain somente leitura e scratch tmpfs de 64 MiB.
Houve teste prévio dessas proteções, e limites de tempo, CPU, memória virtual, processos, arquivos e descritores.
Loopback isolado foi permitido para fixtures; saída externa negada. Nenhum script suspeito foi executado no PC do usuário.
Os logs e runners estão em `security-review/hardened.2/`. O runner registra caminhos do ambiente desta revisão e não é um instalador a executar no Windows.

Os resultados de hardened.1, incluindo suas antigas suítes Rust/browser, são históricos e não contam como validação desta rodada.
A equivalência do patch e a integridade do pacote estão registradas separadamente em `security-review/hardened.2/patch-verification.json` e no manifesto.

## 5. Uso e risco residual

Siga [o guia Windows](docs/install-windows-hardened.md) usando este código-fonte. Não use `npx pandacode`, Homebrew ou o instalador público upstream para obter hardened.2.
Mantenha o Avast ativo e não adicione exceções ao PowerShell/aplicativo para contornar o bloqueio.
A nova política reduz caminhos automáticos identificados; ela não garante que o alerta deixe de ocorrer.

A compilação pode baixar dependências/ferramentas e executar código delas, incluindo `build.rs`; a versão não é offline por definição.
Não foi feita uma nova varredura de vulnerabilidades de todas as dependências nesta rodada.
Depois de instalado, prompts/código e resultados podem ser enviados ao provedor de IA escolhido; comandos autorizados, bots, nuvem e integrações preservadas também podem ter rede.
Este hardening não impõe um firewall aos agentes nem impede todo comando malicioso gerado por prompt injection.
A versão interna 0.3.28 e o identificador do app foram mantidos; faça backup dos dados antes de substituir uma instalação existente.

Nenhum fonte/código/credencial foi enviado a scanner público. As consultas externas desta revisão foram de documentação pública.
Para atribuir a detecção, é necessário preservar o evento original, a linha bloqueada e seu processo pai, com segredos redigidos, e o hash do instalador usado. Não reproduza a instalação liberando o bloqueio.

## 6. Fontes e trilha de evidência

- [Tauri: instalação Windows e modos WebView2](https://v2.tauri.app/distribute/windows-installer/): `skip` não instala o runtime; aplicativo depende dele.
- [npm v10: ignore-scripts](https://docs.npmjs.com/cli/v10/using-npm/config/#ignore-scripts): hooks e comandos explicitamente solicitados têm comportamentos distintos.
- [Cargo build: locked](https://doc.rust-lang.org/cargo/commands/cargo-build.html): resolução fixada não equivale a `offline` ou a sandbox.
- `security-review/hardened.2/findings.json`: constatações, escopo e estado da atribuição.
- `security-review/hardened.2/verification.json`: resultados atuais e validações não executadas.
- `security-review/hardened.2/changes.patch`: diff dos arquivos de produto/docs/testes; exclui apenas a árvore de evidências `security-review/` para evitar recursão.
- `security-review/hardened.2/changes.json`: inventário detalhado do delta.
- `security-review/hardened.2/manifest.json`: hashes dos arquivos finais, exceto o próprio manifesto.

**Conclusão:** correções de instalação entregues com testes locais e limites registrados. Sem atestado de ausência de malware, sem aprovação Windows/Avast e sem causa confirmada para o alerta original.
