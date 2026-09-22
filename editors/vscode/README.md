# Extensão DartForge para VS Code

Cliente TypeScript **fino** do servidor `dartforge-lsp` (Rust): só localiza
o binário e o inicia por stdio. Toda a lógica — sincronização incremental,
conversão UTF-16, diagnósticos — vive no servidor (`crates/lsp`). A
extensão ser TypeScript é necessidade da plataforma (o host do VS Code
executa JavaScript), registrada no PLANO.md como precisão obrigatória.

## Pré-requisitos

- Node 20+ e npm.
- O binário `dartforge-lsp` construído e no `PATH`, ou o caminho dele na
  configuração `dartforge.serverPath`:

```powershell
cargo build --release -p dartforge-lsp
# o binário fica em target\release\dartforge-lsp.exe
```

## Compilar a extensão

```powershell
cd editors\vscode
npm install
npm run compile
```

## Rodar em modo de desenvolvimento (F5)

1. Abra a pasta `editors/vscode` no VS Code.
2. Pressione F5 (ou *Run and Debug › Run Extension*): abre uma janela de
   desenvolvimento com a extensão carregada.
3. Nessa janela, abra um arquivo `.dart` (por exemplo do
   `C:/MyDartProjects/new_sali` com um erro de sintaxe inserido, como
   `int x = ;` dentro de uma função): o servidor publica
   `textDocument/publishDiagnostics` e o editor sublinha o erro com
   `source: "dartforge"`.
4. O canal *Output › DartForge* mostra o ciclo de vida (`initialize`,
   `didOpen`, `publishDiagnostics`); com `dartforge.trace.server` em
   `messages` ou `verbose`, cada mensagem do protocolo.

## Configurações

| Chave | Padrão | Efeito |
| --- | --- | --- |
| `dartforge.serverPath` | `null` (usa o `PATH`) | Caminho do binário `dartforge-lsp` |
| `dartforge.trace.server` | `off` | Rastreia mensagens cliente ↔ servidor |

## O que a extensão NÃO faz (de propósito)

Nenhuma análise no cliente: sem parsing, sem diagnósticos próprios, sem
hover/completion/definição (capacidades não implementadas no servidor neste
brief — ver `docs/LSP.md`). Se o binário não for encontrado, o VS Code
mostra o erro de ativação no canal da extensão.
