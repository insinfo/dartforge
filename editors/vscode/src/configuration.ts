// Client settings forwarded to the server.
//
// The server asks for the `dart` section through `workspace/configuration`
// (the keys of the Dart analysis server, `crates/lsp/src/registration.rs`).
// The extension contributes the same keys under `dartforge.*`, so that they
// exist without the Dart extension installed. The answer for `dart` merges
// both: a `dartforge.*` value set by the user wins, then a `dart.*` value,
// then the `dartforge.*` default.
import * as vscode from "vscode";

/// `dartforge.*` keys sent to the server under the same name in `dart`.
export const FORWARDED_KEYS = [
    "lineLength",
    "enableSdkFormatter",
    "completeFunctionCalls",
    "enableSnippets",
    "showTodos",
    "documentation",
    "updateImportsOnRename",
    "renameFilesWithClasses",
    "analysisExcludedFolders",
    "maxCompletionItems",
] as const;

/// Whether the user set `key` at any level (not only the default).
function isSet(configuration: vscode.WorkspaceConfiguration, key: string): boolean {
    const inspected = configuration.inspect(key);
    return (
        inspected !== undefined &&
        (inspected.globalValue !== undefined ||
            inspected.workspaceValue !== undefined ||
            inspected.workspaceFolderValue !== undefined)
    );
}

/// The `dart` section for `scopeUri`, given what VS Code holds for
/// `dart` (`base`, possibly empty when the Dart extension is not installed).
export function dartSection(base: unknown, scopeUri: string | undefined): Record<string, unknown> {
    const result: Record<string, unknown> =
        base !== null && typeof base === "object" ? { ...(base as Record<string, unknown>) } : {};
    let scope: vscode.Uri | undefined;
    try {
        scope = scopeUri ? vscode.Uri.parse(scopeUri) : undefined;
    } catch {
        scope = undefined;
    }
    const ours = vscode.workspace.getConfiguration("dartforge", scope);
    for (const key of FORWARDED_KEYS) {
        if (isSet(ours, key) || result[key] === undefined || result[key] === null) {
            const value = ours.get(key);
            if (value === null || value === undefined) {
                delete result[key];
            } else {
                result[key] = value;
            }
        }
    }
    return result;
}

/// `initializationOptions` the server reads at startup
/// (`crates/lsp/src/server.rs`, `initialize`).
export function initializationOptions(): Record<string, unknown> {
    const configuration = vscode.workspace.getConfiguration("dartforge");
    return {
        suggestFromUnimportedLibraries: configuration.get<boolean>("suggestFromUnimportedLibraries", true),
        completeFunctionCalls: configuration.get<boolean>("completeFunctionCalls", true),
        renameFilesWithClasses: configuration.get<string>("renameFilesWithClasses", "never"),
        // The server's `dart/textDocument/publishClosingLabels` and
        // `publishOutline` notifications have no consumer in this client.
        closingLabels: false,
        outline: false,
    };
}

/// Settings whose change requires a new server process.
export const RESTART_SETTINGS = [
    "dartforge.serverPath",
    "dartforge.sdkPath",
    "dartforge.server.extraEnv",
    "dartforge.suggestFromUnimportedLibraries",
];
