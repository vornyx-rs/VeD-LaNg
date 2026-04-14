"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const path = __importStar(require("path"));
const cp = __importStar(require("child_process"));
function getConfig() {
    return vscode.workspace.getConfiguration('ved');
}
function getCompilerPath() {
    return getConfig().get('compilerPath', 'vedc');
}
function runVedCommand(args, cwd, outputChannel) {
    const compiler = getCompilerPath();
    const verbose = getConfig().get('verbose', false);
    const fullArgs = verbose ? [...args, '--verbose'] : args;
    outputChannel.show(true);
    outputChannel.appendLine(`> ${compiler} ${fullArgs.join(' ')}`);
    const proc = cp.spawn(compiler, fullArgs, { cwd, shell: false });
    proc.stdout.on('data', (data) => {
        outputChannel.append(data.toString());
    });
    proc.stderr.on('data', (data) => {
        outputChannel.append(data.toString());
    });
    proc.on('error', (err) => {
        outputChannel.appendLine(`error: could not launch '${compiler}': ${err.message}`);
        outputChannel.appendLine(`Install vedc and ensure it is on your PATH, or set ved.compilerPath.`);
    });
    proc.on('close', (code) => {
        outputChannel.appendLine(`\nProcess exited with code ${code}`);
    });
}
function activate(context) {
    const outputChannel = vscode.window.createOutputChannel('VED');
    context.subscriptions.push(outputChannel);
    function activeFile() {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'ved') {
            vscode.window.showWarningMessage('Open a .ved file first.');
            return null;
        }
        const filePath = editor.document.uri.fsPath;
        const cwd = path.dirname(filePath);
        return { filePath, cwd };
    }
    context.subscriptions.push(vscode.commands.registerCommand('ved.runFile', () => {
        const info = activeFile();
        if (!info)
            return;
        runVedCommand(['run', info.filePath], info.cwd, outputChannel);
    }));
    context.subscriptions.push(vscode.commands.registerCommand('ved.buildFile', () => {
        const info = activeFile();
        if (!info)
            return;
        const target = getConfig().get('defaultTarget', 'auto');
        const outDir = getConfig().get('outputDir', 'dist');
        runVedCommand(['build', info.filePath, '--target', target, '--out', outDir], info.cwd, outputChannel);
    }));
    context.subscriptions.push(vscode.commands.registerCommand('ved.checkFile', () => {
        const info = activeFile();
        if (!info)
            return;
        runVedCommand(['check', info.filePath], info.cwd, outputChannel);
    }));
    context.subscriptions.push(vscode.commands.registerCommand('ved.buildWeb', () => {
        const info = activeFile();
        if (!info)
            return;
        const outDir = getConfig().get('outputDir', 'dist');
        runVedCommand(['build', info.filePath, '--target', 'web', '--out', outDir], info.cwd, outputChannel);
    }));
    context.subscriptions.push(vscode.commands.registerCommand('ved.buildServer', () => {
        const info = activeFile();
        if (!info)
            return;
        const outDir = getConfig().get('outputDir', 'dist');
        runVedCommand(['build', info.filePath, '--target', 'server', '--out', outDir], info.cwd, outputChannel);
    }));
    context.subscriptions.push(vscode.commands.registerCommand('ved.buildNative', () => {
        const info = activeFile();
        if (!info)
            return;
        const outDir = getConfig().get('outputDir', 'dist');
        runVedCommand(['build', info.filePath, '--target', 'bin', '--out', outDir], info.cwd, outputChannel);
    }));
}
function deactivate() { }
//# sourceMappingURL=extension.js.map