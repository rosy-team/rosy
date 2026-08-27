const os = require("os");
const path = require("path");
const { workspace, window } = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

/**
 * Resolve leading ~ to the user's home directory.
 * VS Code's extension host does not expand shell shortcuts consistently.
 */
function resolvePath(p) {
  if (p.startsWith("~/") || p === "~") {
    return path.join(os.homedir(), p.slice(1));
  }
  return p;
}

function activate(context) {
  const config = workspace.getConfiguration("rosy");
  const serverPath = resolvePath(config.get("lspPath", "rosy"));

  const serverOptions = {
    command: serverPath,
    args: ["lsp"],
    transport: TransportKind.stdio,
    options: { env: process.env },
  };

  const clientOptions = {
    documentSelector: [
      { scheme: "file", language: "rosy" },
    ],
  };

  client = new LanguageClient(
    "rosy-lsp",
    "ROSY Language Server",
    serverOptions,
    clientOptions
  );

  client.start();
}

function deactivate() {
  if (client) {
    return client.stop();
  }
}

module.exports = { activate, deactivate };
