# DartForge extension for VS Code

A **thin** TypeScript client of the `dartforge-lsp` server (Rust): it only
locates the binary and starts it over stdio. All the logic — incremental
synchronization, UTF-16 conversion, diagnostics — lives in the server
(`crates/lsp`). The extension being TypeScript is a platform requirement (the
VS Code host runs JavaScript), recorded in PLANO.md as a mandatory exception.

## Prerequisites

- Node 20+ and npm.
- The `dartforge-lsp` binary built and on the `PATH`, or its path in the
  `dartforge.serverPath` setting:

```powershell
cargo build --release -p dartforge-lsp
# the binary ends up in target\release\dartforge-lsp.exe
```

## Building the extension

```powershell
cd editors\vscode
npm install
npm run compile
```

## Running in development mode (F5)

1. Open the `editors/vscode` folder in VS Code.
2. Press F5 (or *Run and Debug › Run Extension*): it opens a development
   window with the extension loaded.
3. In that window, open a `.dart` file (for example from
   `C:/MyDartProjects/new_sali` with a syntax error inserted, like
   `int x = ;` inside a function): the server publishes
   `textDocument/publishDiagnostics` and the editor underlines the error with
   `source: "dartforge"`.
4. The *Output › DartForge* channel shows the life cycle (`initialize`,
   `didOpen`, `publishDiagnostics`); with `dartforge.trace.server` set to
   `messages` or `verbose`, every protocol message.

## Settings

| Key | Default | Effect |
| --- | --- | --- |
| `dartforge.serverPath` | `null` (uses the `PATH`) | Path of the `dartforge-lsp` binary |
| `dartforge.trace.server` | `off` | Traces client ↔ server messages |

## What the server offers

Everything comes from the server through the protocol; `vscode-languageclient`
enables each capability the server announces in `initialize`, with no code in
the extension: diagnostics (immediate and typed), completion (with automatic
import and call snippets), hover, go to definition, to the type definition and
to the implementations, references, document highlights, rename (with
`prepareRename`), quick fixes and assists, signature help, document and
workspace symbols, folding, selection ranges, inlay hints and type hierarchy.
The measured parity of each one with Dart's server is in `docs/LSP.md`
("Paridade com o servidor do Dart").

## What the extension does NOT do (on purpose)

No analysis in the client: no parsing, no diagnostics of its own and no
features implemented in TypeScript. Formatting is left to `dart format`
(decision in `docs/LSP.md`). If the binary is not found, VS Code shows the
activation error in the extension's channel.
