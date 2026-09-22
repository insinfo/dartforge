// Cliente fino do servidor DartForge: só localiza o binário e o inicia.
//
// Toda a lógica vive no servidor Rust (`crates/lsp`, binário
// `dartforge-lsp`); aqui não há análise, diagnóstico nem conversão de
// posição — só o transporte stdio via `vscode-languageclient`, como o
// cliente do rust-analyzer (`references/rust-analyzer/editors/code`).
import * as vscode from "vscode";
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from "vscode-languageclient/node";

let cliente: LanguageClient | undefined;

/// Localiza o binário: configuração `dartforge.serverPath`, senão `PATH`.
async function caminhoDoServidor(): Promise<string> {
    const configurado = vscode.workspace
        .getConfiguration("dartforge")
        .get<string | null>("serverPath");
    if (configurado && configurado.trim() !== "") {
        return configurado;
    }
    return "dartforge-lsp";
}

export async function activate(contexto: vscode.ExtensionContext): Promise<void> {
    const comando = await caminhoDoServidor();
    const servidor: ServerOptions = {
        command: comando,
        args: ["--stdio"],
        transport: TransportKind.stdio,
    };
    const opcoes: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "dart" }],
        traceOutputChannel: vscode.window.createOutputChannel("DartForge"),
    };
    cliente = new LanguageClient("dartforge", "DartForge", servidor, opcoes);
    contexto.subscriptions.push(cliente);
    await cliente.start();
}

export async function deactivate(): Promise<void> {
    if (cliente) {
        await cliente.stop();
        cliente = undefined;
    }
}
