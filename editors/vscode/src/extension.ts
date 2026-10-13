// Thin client of the DartForge server: it only locates the binary and starts it.
//
// All the logic lives in the Rust server (`crates/lsp`, binary
// `dartforge-lsp`); there is no analysis, diagnostics or position conversion
// here — only the stdio transport through `vscode-languageclient`, like the
// rust-analyzer client (`references/rust-analyzer/editors/code`).
import * as vscode from "vscode";
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

/// Locates the binary: the `dartforge.serverPath` setting, otherwise `PATH`.
async function serverPath(): Promise<string> {
    const configured = vscode.workspace
        .getConfiguration("dartforge")
        .get<string | null>("serverPath");
    if (configured && configured.trim() !== "") {
        return configured;
    }
    return "dartforge-lsp";
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    const command = await serverPath();
    const server: ServerOptions = {
        command,
        args: ["--stdio"],
        transport: TransportKind.stdio,
    };
    const options: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "dart" }],
        traceOutputChannel: vscode.window.createOutputChannel("DartForge"),
    };
    client = new LanguageClient("dartforge", "DartForge", server, options);
    context.subscriptions.push(client);
    await client.start();
}

export async function deactivate(): Promise<void> {
    if (client) {
        await client.stop();
        client = undefined;
    }
}
