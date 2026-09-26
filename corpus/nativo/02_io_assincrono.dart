// dart:io assíncrono (IOService): leitura e escrita, listagem recursiva, stat, RandomAccessFile, links, cópia, exitCode.
import 'dart:io';
import 'dart:convert';

Future<void> main() async {
  final d = await Directory.systemTemp.createTemp('dfio2');
  final f = File('${d.path}/x.txt');
  await f.writeAsString('abc\ndef\n');
  print(await f.exists());
  print(await f.length());
  print(await f.readAsString());
  print(await f.readAsLines());
  final bytes = await f.readAsBytes();
  print(bytes.length);
  await File('${d.path}/y.bin').writeAsBytes([1, 2, 3, 250]);
  print(await File('${d.path}/y.bin').readAsBytes());
  await Directory('${d.path}/sub/a/b').create(recursive: true);
  await File('${d.path}/sub/a/b/z.txt').writeAsString('z');
  final nomes = <String>[];
  await for (final e in d.list(recursive: true)) {
    nomes.add('${e.runtimeType}:${e.path.substring(d.path.length)}');
  }
  nomes.sort();
  print(nomes);
  final st = await f.stat();
  print('${st.type} ${st.size} ${st.mode.toRadixString(8)}');
  print(FileStat.statSync(d.path).type);
  final raf = await f.open(mode: FileMode.append);
  await raf.writeString('ghi\n');
  print(await raf.position());
  await raf.close();
  print(f.readAsStringSync());
  final raf2 = f.openSync();
  print(raf2.readSync(3));
  print(utf8.decode(raf2.readSync(100)));
  raf2.setPositionSync(1);
  print(raf2.readByteSync());
  raf2.closeSync();
  try {
    await File('${d.path}/nao/existe').readAsString();
  } on FileSystemException catch (e) {
    print('async erro: ${e.message} | ${e.osError?.message} ${e.osError?.errorCode}');
  }
  try {
    Directory('${d.path}/naoexiste').listSync();
  } on FileSystemException catch (e) {
    print('list erro: ${e.message} ${e.osError?.errorCode}');
  }
  final c = await f.copy('${d.path}/copia.txt');
  print(await c.readAsString() == await f.readAsString());
  await Link('${d.path}/ln').create('${d.path}/x.txt');
  print(await FileSystemEntity.isLink('${d.path}/ln'));
  print((await Link('${d.path}/ln').target()).endsWith('x.txt'));
  print(await FileSystemEntity.identical(f.path, '${d.path}/x.txt'));
  print(f.resolveSymbolicLinksSync() == File('${d.path}/ln').resolveSymbolicLinksSync());
  final l = d.listSync(recursive: true, followLinks: false).map((e) => '${e.runtimeType}:${e.path.substring(d.path.length)}').toList()..sort();
  print(l);
  await d.delete(recursive: true);
  print(await d.exists());
  print(Platform.environment['HOME'] != null);
  print(Platform.script.scheme);
  stdout.write('sem nova linha ');
  stdout.writeln('e com');
  exitCode = 3;
  print(exitCode);
  exitCode = 0;
}
