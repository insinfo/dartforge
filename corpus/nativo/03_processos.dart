// dart:io, processos: Process.run/runSync/start, stdin/stdout de filho, erro de exec, killPid, ambiente e diretório.
import 'dart:io';
import 'dart:convert';

Future<void> main() async {
  final r = await Process.run('echo', ['ola', 'mundo']);
  print('run: ${r.exitCode} [${r.stdout}] [${r.stderr}]');
  final s = Process.runSync('sh', ['-c', 'echo saida; echo erro >&2; exit 3']);
  print('runSync: ${s.exitCode} [${s.stdout}] [${s.stderr}]');
  final p = await Process.start('cat', []);
  p.stdin.writeln('linha 1');
  p.stdin.writeln('linha 2');
  await p.stdin.close();
  final out = await p.stdout.transform(utf8.decoder).join();
  print('cat: [$out] ${await p.exitCode}');
  try {
    await Process.run('/nao/existe', []);
  } on ProcessException catch (e) {
    print('erro: ${e.executable} ${e.errorCode} ${e.message}');
  }
  final k = await Process.start('sleep', ['10']);
  print(Process.killPid(k.pid));
  print('morto: ${await k.exitCode}');
  final amb = await Process.run('sh', ['-c', 'echo \$X'], environment: {'X': 'valor'});
  print(amb.stdout.trim());
  final dir = await Process.run('pwd', [], workingDirectory: '/tmp');
  print(dir.stdout.trim());
}
