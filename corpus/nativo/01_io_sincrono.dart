// dart:io síncrono: stdout/stderr, Platform, arquivos, diretórios e erros do sistema.
import 'dart:io';

void main() {
  stdout.writeln('ola stdout');
  stderr.writeln('ola stderr');
  print(Platform.pathSeparator);
  print(Platform.operatingSystem);
  print(Platform.isLinux);
  print(Platform.numberOfProcessors > 0);
  final d = Directory.systemTemp.createTempSync('dfio');
  final f = File('${d.path}/a.txt');
  f.writeAsStringSync('linha1\nlinha2\n');
  print(f.existsSync());
  print(f.lengthSync());
  print(f.readAsStringSync());
  print(f.readAsLinesSync());
  f.writeAsStringSync('mais\n', mode: FileMode.append);
  print(f.readAsBytesSync().length);
  final g = f.renameSync('${d.path}/b.txt');
  print(g.path.endsWith('b.txt'));
  print(d.listSync().map((e) => e.path.split('/').last).toList());
  print(FileSystemEntity.typeSync(d.path));
  try {
    File('${d.path}/nao').readAsStringSync();
  } on FileSystemException catch (e) {
    print('erro: ${e.message} ${e.osError?.errorCode}');
  }
  d.deleteSync(recursive: true);
  print(d.existsSync());
  exitCode = 0;
}
