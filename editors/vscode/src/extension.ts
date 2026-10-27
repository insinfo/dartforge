// Client of the DartForge language server.
//
// All the language logic lives in the Rust server (`crates/lsp`, binary
// `dartforge-lsp`); the extension contributes the `dart` language (grammar
// and editor configuration), locates and starts the binary over stdio
// through `vscode-languageclient`, forwards the settings the server reads,
// and offers the restart and output commands.
import * as vscode from "vscode";
import { LanguageClient, LanguageClientOptions, ServerOptions, State, TransportKind } from "vscode-languageclient/node";
import { RESTART_SETTINGS, dartSection, initializationOptions } from "./configuration";
import { ServerNotFoundError, resolveServer, serverEnvironment } from "./server";

/// Identifier of the Dart extension, which ships its own language server.
const DART_CODE_ID = "Dart-Code.dart-code";
/// `globalState` key: the duplication notice was already shown.
const DART_CODE_NOTICE_KEY = "dartforge.dartCodeNoticeShown";

let client: LanguageClient | undefined;
let output: vscode.OutputChannel;
let extensionContext: vscode.ExtensionContext;
/// Serializes start, stop and restart so that commands and setting changes
/// never race each other.
let pending: Promise<void> = Promise.resolve();
/// Set synchronously when a settings change is about to stop or restart the
/// server, so that the client does not forward that change to a server that is
/// going away (VS Code calls this extension's listener before the client's).
let serverChangePending = false;

function enqueue(task: () => Promise<void>): Promise<void> {
    pending = pending.then(task).catch((error) => {
        log(`Unexpected error: ${error instanceof Error ? (error.stack ?? error.message) : String(error)}`);
    });
    return pending;
}

function isEnabled(): boolean {
    return vscode.workspace.getConfiguration("dartforge").get<boolean>("enable", true);
}

function log(message: string): void {
    output.appendLine(`[client] ${message}`);
}

/// Shows an error with buttons to open the setting and the output.
async function showStartError(message: string, setting: string): Promise<void> {
    const openSettings = "Open Settings";
    const showOutput = "Show Output";
    const choice = await vscode.window.showErrorMessage(message, openSettings, showOutput);
    if (choice === openSettings) {
        await vscode.commands.executeCommand("workbench.action.openSettings", setting);
    } else if (choice === showOutput) {
        output.show(true);
    }
}

async function startClient(): Promise<void> {
    if (client || !isEnabled()) {
        return;
    }
    let command: string;
    try {
        const server = resolveServer(extensionContext.extensionPath);
        command = server.path;
        log(`Starting ${server.path} (found via ${server.source}).`);
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        log(message);
        if (error instanceof ServerNotFoundError) {
            void showStartError(message, "dartforge.serverPath");
            return;
        }
        throw error;
    }
    const { env, warnings } = serverEnvironment();
    for (const warning of warnings) {
        log(warning);
        void vscode.window.showWarningMessage(warning);
    }
    // `TransportKind.stdio` adds the `--stdio` argument the binary accepts.
    const serverOptions: ServerOptions = {
        command,
        transport: TransportKind.stdio,
        options: { env },
    };
    // Assigned right below; the middleware only runs once the client exists.
    let created: LanguageClient;
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "dart" }],
        outputChannel: output,
        traceOutputChannel: output,
        initializationOptions: initializationOptions(),
        middleware: {
            workspace: {
                // A settings change that restarts the server must not reach the
                // client being stopped.
                didChangeConfiguration: async (sections, next) => {
                    if (created.state === State.Running && !serverChangePending) {
                        await next(sections);
                    }
                },
                configuration: async (params, token, next) => {
                    const result = await next(params, token);
                    if (!Array.isArray(result)) {
                        return result;
                    }
                    return params.items.map((item, i) =>
                        item.section === "dart" ? dartSection(result[i], item.scopeUri) : result[i],
                    );
                },
            },
        },
    };
    created = new LanguageClient("dartforge", "DartForge", serverOptions, clientOptions);
    created.onDidChangeState((event) => {
        if (event.newState === State.Stopped && client === created) {
            log("The server stopped.");
        }
    });
    client = created;
    try {
        await created.start();
        const info = created.initializeResult?.serverInfo;
        log(`Connected to ${info?.name ?? "dartforge-lsp"} ${info?.version ?? ""}`.trimEnd() + ".");
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        log(`The server failed to start: ${message}`);
        client = undefined;
        try {
            await created.dispose();
        } catch {
            // Already stopped.
        }
        // `vscode-languageclient` already notifies the failure; the details are in
        // the output channel.
    }
}

async function stopClient(): Promise<void> {
    const current = client;
    client = undefined;
    if (!current) {
        return;
    }
    try {
        await current.stop();
    } catch (error) {
        log(`Stopping the server failed: ${error instanceof Error ? error.message : String(error)}`);
    }
    await current.dispose();
}

function restartClient(): Promise<void> {
    return enqueue(async () => {
        await stopClient();
        if (!isEnabled()) {
            void vscode.window.showInformationMessage(
                'The DartForge server is disabled ("dartforge.enable" is false).',
            );
            return;
        }
        await startClient();
    });
}

/// When the Dart extension is active too, both servers analyze the same files
/// and every diagnostic and completion shows up twice. Explains it once.
async function noticeDartCode(): Promise<void> {
    if (!vscode.extensions.getExtension(DART_CODE_ID) || !isEnabled()) {
        return;
    }
    if (extensionContext.globalState.get<boolean>(DART_CODE_NOTICE_KEY)) {
        return;
    }
    await extensionContext.globalState.update(DART_CODE_NOTICE_KEY, true);
    const disableHere = "Disable DartForge Server Here";
    const openSettings = "Open Settings";
    const choice = await vscode.window.showInformationMessage(
        "The Dart extension (Dart-Code) is also enabled. Both run a Dart language server, so diagnostics, " +
            'completions and code actions appear twice. Set "dartforge.enable" to false to keep only ' +
            "DartForge's syntax highlighting, or disable the Dart extension for this workspace.",
        disableHere,
        openSettings,
    );
    if (choice === disableHere) {
        const target = vscode.workspace.workspaceFolders
            ? vscode.ConfigurationTarget.Workspace
            : vscode.ConfigurationTarget.Global;
        await vscode.workspace.getConfiguration("dartforge").update("enable", false, target);
    } else if (choice === openSettings) {
        await vscode.commands.executeCommand("workbench.action.openSettings", "dartforge.enable");
    }
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    extensionContext = context;
    output = vscode.window.createOutputChannel("DartForge");
    context.subscriptions.push(
        output,
        vscode.commands.registerCommand("dartforge.restartServer", restartClient),
        vscode.commands.registerCommand("dartforge.showOutput", () => output.show(true)),
        vscode.workspace.onDidChangeConfiguration((event) => {
            if (event.affectsConfiguration("dartforge.enable")) {
                serverChangePending = !isEnabled();
                void enqueue(async () => {
                    if (isEnabled()) {
                        await startClient();
                    } else {
                        log('Stopping the server: "dartforge.enable" is false.');
                        await stopClient();
                    }
                }).finally(() => (serverChangePending = false));
            } else if (isEnabled() && RESTART_SETTINGS.some((s) => event.affectsConfiguration(s))) {
                serverChangePending = true;
                log("Restarting the server after a settings change.");
                void restartClient().finally(() => (serverChangePending = false));
            }
        }),
        vscode.extensions.onDidChange(() => void noticeDartCode()),
    );
    void noticeDartCode();
    await enqueue(startClient);
}

export async function deactivate(): Promise<void> {
    await enqueue(stopClient);
}
