import * as vscode from 'vscode';
import * as path from 'path';
import * as cp from 'child_process';

function getConfig(): vscode.WorkspaceConfiguration {
  return vscode.workspace.getConfiguration('ved');
}

function getCompilerPath(): string {
  return getConfig().get<string>('compilerPath', 'vedc');
}

function runVedCommand(
  args: string[],
  cwd: string,
  outputChannel: vscode.OutputChannel
): void {
  const compiler = getCompilerPath();
  const verbose = getConfig().get<boolean>('verbose', false);
  const fullArgs = verbose ? [...args, '--verbose'] : args;

  outputChannel.show(true);
  outputChannel.appendLine(`> ${compiler} ${fullArgs.join(' ')}`);

  const proc = cp.spawn(compiler, fullArgs, { cwd, shell: false });

  proc.stdout.on('data', (data: Buffer) => {
    outputChannel.append(data.toString());
  });

  proc.stderr.on('data', (data: Buffer) => {
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

export function activate(context: vscode.ExtensionContext): void {
  const outputChannel = vscode.window.createOutputChannel('VED');
  context.subscriptions.push(outputChannel);

  function activeFile(): { filePath: string; cwd: string } | null {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'ved') {
      vscode.window.showWarningMessage('Open a .ved file first.');
      return null;
    }
    const filePath = editor.document.uri.fsPath;
    const cwd = path.dirname(filePath);
    return { filePath, cwd };
  }

  context.subscriptions.push(
    vscode.commands.registerCommand('ved.runFile', () => {
      const info = activeFile();
      if (!info) return;
      runVedCommand(['run', info.filePath], info.cwd, outputChannel);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('ved.buildFile', () => {
      const info = activeFile();
      if (!info) return;
      const target = getConfig().get<string>('defaultTarget', 'auto');
      const outDir = getConfig().get<string>('outputDir', 'dist');
      runVedCommand(
        ['build', info.filePath, '--target', target, '--out', outDir],
        info.cwd,
        outputChannel
      );
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('ved.checkFile', () => {
      const info = activeFile();
      if (!info) return;
      runVedCommand(['check', info.filePath], info.cwd, outputChannel);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('ved.buildWeb', () => {
      const info = activeFile();
      if (!info) return;
      const outDir = getConfig().get<string>('outputDir', 'dist');
      runVedCommand(
        ['build', info.filePath, '--target', 'web', '--out', outDir],
        info.cwd,
        outputChannel
      );
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('ved.buildServer', () => {
      const info = activeFile();
      if (!info) return;
      const outDir = getConfig().get<string>('outputDir', 'dist');
      runVedCommand(
        ['build', info.filePath, '--target', 'server', '--out', outDir],
        info.cwd,
        outputChannel
      );
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('ved.buildNative', () => {
      const info = activeFile();
      if (!info) return;
      const outDir = getConfig().get<string>('outputDir', 'dist');
      runVedCommand(
        ['build', info.filePath, '--target', 'bin', '--out', outDir],
        info.cwd,
        outputChannel
      );
    })
  );
}

export function deactivate(): void {}
