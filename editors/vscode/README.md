# DartForge for VS Code

Dart language support backed by the DartForge language server,
`dartforge-lsp` (Rust, `crates/lsp`). The extension works on its own: it
contributes the `dart` language (file association, TextMate grammar, comment,
bracket, indentation and doc-comment rules) and starts the server over stdio.
All the analysis lives in the server; the TypeScript client only locates the
binary, forwards settings and offers a few commands.

## Features

Everything below comes from the server. The client advertises the standard
LSP client capabilities through `vscode-languageclient`, and the server
announces (statically in `initialize`, or by dynamic registration after
`initialized`):

- **Diagnostics**: syntax errors immediately, typed (semantic) diagnostics
  from a background analysis; optional `TODO` diagnostics.
- **Completion** with resolve, automatic imports from unimported libraries,
  call snippets with argument placeholders and code snippets.
- **Hover**, **signature help** (`(` and `,`).
- **Navigation**: go to definition, type definition and implementations,
  find references, document highlights, call hierarchy and type hierarchy.
- **Symbols**: document outline and workspace symbol search.
- **Editing**: rename (with `prepareRename`), quick fixes, assists and
  refactorings (`quickfix`, `refactor`, `source.organizeImports`,
  `source.fixAll`, `source.sortMembers`), import updates when Dart files are
  renamed or moved (`workspace/willRenameFiles`).
- **Formatting**: document, range and on-type formatting (`}` and `;`) with
  the server's port of `dart_style`, honouring the line length setting.
- **Presentation**: semantic tokens (full and range), folding ranges,
  selection ranges, inlay hints, document links, code lenses and color
  decorators.

The measured parity of each feature with the Dart SDK's analysis server is in
`docs/LSP.md` ("Paridade com o servidor do Dart").

## Requirements

- The `dartforge-lsp` binary. Build it from the repository root:

  ```sh
  cargo build --release -p dartforge-lsp
  # -> target/release/dartforge-lsp (target\release\dartforge-lsp.exe on Windows)
  ```

- A Dart SDK (3.6.x is the reference version). The server looks for it in
  `dartforge.sdkPath`, then `DARTFORGE_SDK_LIB`, the SDK of the DartForge
  distribution, `DART_SDK`, and finally the `dart` executable on the `PATH`.
  Without an SDK only syntax diagnostics, formatting and the outline work: hover,
  navigation, completion, semantic tokens and typed diagnostics need the
  SDK's libraries.

The extension finds the server in this order:

1. the `dartforge.serverPath` setting;
2. a binary bundled in the extension, `server/dartforge-lsp`
   (`server/dartforge-lsp.exe` on Windows);
3. `dartforge-lsp` on the `PATH`.

If none is found, or the configured file does not exist or is not executable,
the extension shows an error naming `dartforge.serverPath` and does not start
the client.

## Build, package and install

Node 20 or newer and npm are needed (commands for macOS/Linux; on Windows
PowerShell the same commands work, using `\` in paths and `Copy-Item`
instead of `cp`):

```sh
cd editors/vscode
npm install
npm run compile        # type-check and emit out/ (used by F5 debugging)
```

To produce a `.vsix` with the server bundled inside:

```sh
cargo build --release -p dartforge-lsp
mkdir -p editors/vscode/server
cp target/release/dartforge-lsp editors/vscode/server/
cd editors/vscode
npm run package        # bundles out/extension.js with esbuild, then runs vsce
code --install-extension dartforge-0.1.0.vsix
```

```powershell
# Windows PowerShell
cargo build --release -p dartforge-lsp
New-Item -ItemType Directory -Force editors\vscode\server | Out-Null
Copy-Item target\release\dartforge-lsp.exe editors\vscode\server\
cd editors\vscode
npm run package
code --install-extension dartforge-0.1.0.vsix
```

A bundled binary only runs on the platform it was built for; for other
platforms package one `.vsix` per target (`vsce package --target ...`) or
leave `server/` empty and rely on `dartforge.serverPath` or the `PATH`.

### Running from source (F5)

1. Open the `editors/vscode` folder in VS Code.
2. Press F5 (*Run Extension*): a development window opens with the extension
   loaded (the `npm: compile` task runs first).
3. Open a `.dart` file and insert an error such as `int x = ;` inside a
   function: the server publishes the diagnostic with source `dartforge`.

## Commands

| Command | Effect |
| --- | --- |
| `DartForge: Restart Server` (`dartforge.restartServer`) | Stops the server and starts it again, locating the binary anew |
| `DartForge: Show Output` (`dartforge.showOutput`) | Shows the *DartForge* output channel |

The *DartForge* output channel holds the client's messages (`[client] ...`),
the server's `stderr` and, with `dartforge.trace.server`, the protocol trace.

## Settings

| Key | Default | Effect |
| --- | --- | --- |
| `dartforge.enable` | `true` | Starts the language server; `false` keeps only the grammar and editor rules |
| `dartforge.serverPath` | `null` | Path of `dartforge-lsp` (`~` and `${workspaceFolder}` are expanded) |
| `dartforge.sdkPath` | `null` | Dart SDK root (containing `lib/libraries.json`), passed as `DARTFORGE_SDK_LIB` |
| `dartforge.server.extraEnv` | `{}` | Extra environment variables, e.g. `{"DARTFORGE_LSP_DEBUG": "1"}` |
| `dartforge.trace.server` | `off` | `messages` or `verbose` traces client/server messages |
| `dartforge.suggestFromUnimportedLibraries` | `true` | Completion from unimported libraries (initialization option) |
| `dartforge.lineLength` | `null` (80) | Formatter line length |
| `dartforge.enableSdkFormatter` | `true` | Server formatting on/off |
| `dartforge.completeFunctionCalls` | `true` | Call completions with parentheses and argument placeholders |
| `dartforge.enableSnippets` | `true` | Code snippets in completion |
| `dartforge.showTodos` | `false` | `TODO` diagnostics: `true` or a list of kinds |
| `dartforge.documentation` | `full` | Documentation in completion items: `full`, `summary`, `none` |
| `dartforge.updateImportsOnRename` | `true` | Updates imports when Dart files are renamed or moved |
| `dartforge.renameFilesWithClasses` | `never` | `always` renames the file with a class of the same name |
| `dartforge.analysisExcludedFolders` | `[]` | Folders the server does not analyze |
| `dartforge.maxCompletionItems` | `null` | Maximum number of completion items |

Changing `serverPath`, `sdkPath`, `server.extraEnv` or
`suggestFromUnimportedLibraries` restarts the server; `enable` starts or stops
it. The other settings are read by the server through
`workspace/configuration`, which asks for the `dart` section (the same keys as
the Dart extension): a `dartforge.*` value you set wins over the matching
`dart.*` value, which wins over the `dartforge.*` default. In untrusted
workspaces `serverPath`, `sdkPath` and `server.extraEnv` are only read from
user settings.

## Using it next to the Dart extension

The Dart extension (`Dart-Code.dart-code`) runs the Dart SDK's analysis
server. With both extensions enabled, every diagnostic, completion and code
action appears twice. When the Dart extension is detected, DartForge shows a
one-time notice. Choose one server per workspace:

- keep the Dart extension: set `"dartforge.enable": false` (DartForge then
  only contributes highlighting, which the Dart extension also provides); or
- keep DartForge: disable the Dart extension for the workspace
  (*Extensions* view, *Disable (Workspace)*).

Both extensions contribute the `dart` language id, which VS Code allows. Both
grammars use the `source.dart` scope, so VS Code uses one of them and themes
apply either way; the server's semantic tokens refine the result.
