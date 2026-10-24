# Windows: compilar e instalar 0.3.30.hardened

Este pacote contém código-fonte, não um instalador Windows pré-compilado.
O executável precisa ser produzido no Windows com os pré-requisitos do Tauri,
Node.js, Rust/MSVC e o runtime Microsoft WebView2 já instalados. O instalador
não baixa WebView2. Sem o runtime, o aplicativo não funciona.

## Comandos locais

No diretório deste projeto, sem elevar o terminal a administrador:

```text
npm ci --ignore-scripts
npm run test:install-security
npm run release:build -- windows-x86_64
```

Para Windows ARM64, use `windows-aarch64` em um host ARM64 compatível.
O build executa explicitamente TypeScript/Vite, Cargo e o empacotador Tauri.
`--ignore-scripts` bloqueia os scripts de instalação do npm, não esses
comandos deliberados nem os `build.rs` das dependências Rust.
A compilação pode baixar dependências/ferramentas públicas; não é um build offline.

Saída x64 esperada do script, ainda não produzida nesta revisão Linux:

```text
release/0.3.30-hardened/hardened/PandaCode-0.3.30.hardened-Windows-x64-setup.exe
release/0.3.30-hardened/hardened/PandaCode-0.3.30.hardened-Windows-x64-setup.exe.sha256
```

Revise o binário produzido, a origem das ferramentas e os alertas do antivírus antes de executá-lo.
Não desative o Avast, não adicione exceção para PowerShell e não ignore uma
nova detecção para continuar a instalação. O hash verifica identidade, não inocuidade.
A ausência de assinatura Authenticode não pode ser corrigida com um certificado inventado.

## O que não usar

Não use `npx pandacode`, Homebrew, links da release upstream ou um instalador
previamente baixado para obter estas correções. Eles são outros artefatos.
Não use `ExecutionPolicy Bypass`, exclusões de antivírus ou comandos remotos
canalizados para um interpretador como atalho de instalação.

## Limites

O identificador nativo foi preservado e a versão interna agora é `0.3.30-hardened`. Faça backup dos
dados existentes antes de substituir uma instalação. A tela About mostra 0.3.30.hardened.
O aplicativo continua executando comandos de IA autorizados e pode abrir o PowerShell;
a configuração não impõe um firewall aos processos dos agentes.
As atualizações do Windows/WebView2 administradas pela Microsoft não foram alteradas.

Referências: [instalação Windows do Tauri](https://v2.tauri.app/distribute/windows-installer/),
[pré-requisitos](https://v2.tauri.app/start/prerequisites/),
[configuração npm](https://docs.npmjs.com/cli/v10/using-npm/config/).
