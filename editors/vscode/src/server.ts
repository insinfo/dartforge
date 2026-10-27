// Locates the `dartforge-lsp` binary the extension launches.
//
// Order: the `dartforge.serverPath` setting, the binary bundled in the
// extension (`server/dartforge-lsp[.exe]`, shipped inside a packaged .vsix),
// then `dartforge-lsp` on the `PATH`. Every candidate is checked to be an
// executable file before the client starts, so a wrong path produces a clear
// message instead of an obscure spawn failure.
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

/// Name of the server executable on this platform.
export const SERVER_EXECUTABLE = process.platform === "win32" ? "dartforge-lsp.exe" : "dartforge-lsp";

/// Where the binary was found, for the output channel.
export type ServerSource = "setting" | "bundled" | "PATH";

export interface ResolvedServer {
    path: string;
    source: ServerSource;
}

/// A binary that could not be located; `message` is shown to the user.
export class ServerNotFoundError extends Error {}

/// Whether `file` is a regular file the current user can execute.
function isExecutableFile(file: string): boolean {
    try {
        if (!fs.statSync(file).isFile()) {
            return false;
        }
        if (process.platform !== "win32") {
            fs.accessSync(file, fs.constants.X_OK);
        }
        return true;
    } catch {
        return false;
    }
}

/// Expands `~`, `${userHome}` and `${workspaceFolder}` and resolves a relative
/// path against the first workspace folder.
export function expandPath(value: string): string {
    const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    let result = value.trim();
    if (result === "~" || result.startsWith("~/") || result.startsWith("~\\")) {
        result = path.join(os.homedir(), result.slice(1));
    }
    result = result.replace(/\$\{userHome\}/g, os.homedir());
    if (folder) {
        result = result.replace(/\$\{workspaceFolder\}/g, folder);
    }
    if (!path.isAbsolute(result) && folder) {
        result = path.join(folder, result);
    }
    return path.normalize(result);
}

/// The binary bundled in the extension, made executable if the archive lost
/// the permission bit; `undefined` when there is none.
function bundledServer(extensionPath: string): string | undefined {
    const file = path.join(extensionPath, "server", SERVER_EXECUTABLE);
    if (!fs.existsSync(file)) {
        return undefined;
    }
    if (!isExecutableFile(file) && process.platform !== "win32") {
        try {
            fs.chmodSync(file, 0o755);
        } catch {
            // Reported below as not executable.
        }
    }
    return isExecutableFile(file) ? file : undefined;
}

/// The first `dartforge-lsp` on the `PATH`.
function serverOnPath(): string | undefined {
    const directories = (process.env.PATH ?? "").split(path.delimiter).filter((d) => d !== "");
    for (const directory of directories) {
        const file = path.join(directory, SERVER_EXECUTABLE);
        if (isExecutableFile(file)) {
            return file;
        }
    }
    return undefined;
}

/// Locates the server or throws [ServerNotFoundError] with a message that
/// names the setting to fix.
export function resolveServer(extensionPath: string): ResolvedServer {
    const configured = vscode.workspace.getConfiguration("dartforge").get<string | null>("serverPath");
    if (configured && configured.trim() !== "") {
        const file = expandPath(configured);
        if (!fs.existsSync(file)) {
            throw new ServerNotFoundError(
                `The DartForge server set in "dartforge.serverPath" does not exist: ${file}`,
            );
        }
        if (!isExecutableFile(file)) {
            throw new ServerNotFoundError(
                `The DartForge server set in "dartforge.serverPath" is not an executable file: ${file}`,
            );
        }
        return { path: file, source: "setting" };
    }
    const bundled = bundledServer(extensionPath);
    if (bundled) {
        return { path: bundled, source: "bundled" };
    }
    const onPath = serverOnPath();
    if (onPath) {
        return { path: onPath, source: "PATH" };
    }
    throw new ServerNotFoundError(
        `The DartForge language server (${SERVER_EXECUTABLE}) was not found. ` +
            `Set "dartforge.serverPath" to the binary or put it on the PATH ` +
            `(build it with "cargo build --release -p dartforge-lsp").`,
    );
}

/// The environment of the server process: the extension host's, the
/// `dartforge.server.extraEnv` variables and, with `dartforge.sdkPath`,
/// `DARTFORGE_SDK_LIB` (the server's first SDK source). Returns warnings for
/// the output channel.
export function serverEnvironment(): { env: NodeJS.ProcessEnv; warnings: string[] } {
    const configuration = vscode.workspace.getConfiguration("dartforge");
    const env: NodeJS.ProcessEnv = { ...process.env };
    const warnings: string[] = [];
    const extra = configuration.get<Record<string, unknown>>("server.extraEnv") ?? {};
    for (const [key, value] of Object.entries(extra)) {
        if (typeof value === "string") {
            env[key] = value;
        } else {
            warnings.push(`Ignoring "dartforge.server.extraEnv.${key}": the value must be a string.`);
        }
    }
    const sdk = configuration.get<string | null>("sdkPath");
    if (sdk && sdk.trim() !== "") {
        const lib = path.join(expandPath(sdk), "lib");
        if (fs.existsSync(path.join(lib, "libraries.json"))) {
            env.DARTFORGE_SDK_LIB = lib;
        } else {
            warnings.push(
                `"dartforge.sdkPath" does not point to a Dart SDK (missing ${path.join(lib, "libraries.json")}); ` +
                    `the server falls back to its own SDK discovery.`,
            );
        }
    }
    return { env, warnings };
}
