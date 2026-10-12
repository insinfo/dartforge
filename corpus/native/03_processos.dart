// dart:io, processos: Process.run/runSync/start, stdin/stdout de filho, erro de exec, killPid, ambiente e diretório.
// Os comandos são os de cada sistema (sh no Unix, cmd no Windows); a saída é comparada com a da VM no mesmo sistema.
import 'dart:io';
import 'dart:convert';

final windows = Platform.isWindows;

/// Um comando de shell: `sh -c` ou `cmd /c`.
List<String> shell(String unix, String win) => windows ? ['cmd', '/c', win] : ['sh', '-c', unix];

Future<void> main() async {
  final eco = windows ? ['cmd', '/c', 'echo ola mundo'] : ['echo', 'ola', 'mundo'];
  final r = await Process.run(eco.first, eco.sublist(1));
  print('run: ${r.exitCode} [${r.stdout.trim()}] [${r.stderr}]');
  final c = shell('echo saida; echo erro >&2; exit 3', 'echo saida& echo erro 1>&2& exit /b 3');
  final s = Process.runSync(c.first, c.sublist(1));
  print('runSync: ${s.exitCode} [${s.stdout.trim()}] [${s.stderr.trim()}]');
  // Um filho que copia a entrada para a saída (`sort` mantém as linhas já
  // ordenadas).
  final p = await Process.start(windows ? 'sort' : 'cat', []);
  p.stdin.writeln('linha 1');
  p.stdin.writeln('linha 2');
  await p.stdin.close();
  final out = await p.stdout.transform(utf8.decoder).join();
  print('copia: [${out.replaceAll('\r\n', '\n')}] ${await p.exitCode}');
  try {
    await Process.run(windows ? r'C:\nao\existe.exe' : '/nao/existe', []);
  } on ProcessException catch (e) {
    print('erro: ${e.executable} ${e.errorCode} ${e.message}');
  }
  final k = windows ? await Process.start('ping', ['-n', '30', '127.0.0.1']) : await Process.start('sleep', ['10']);
  print(Process.killPid(k.pid));
  print('morto: ${await k.exitCode}');
  final a = shell(r'echo $X', 'echo %X%');
  final amb = await Process.run(a.first, a.sublist(1), environment: {'X': 'valor'});
  print(amb.stdout.trim());
  final d = shell('pwd', 'cd');
  final dir = await Process.run(d.first, d.sublist(1), workingDirectory: windows ? Directory.systemTemp.path : '/tmp');
  print(dir.stdout.trim());
}
