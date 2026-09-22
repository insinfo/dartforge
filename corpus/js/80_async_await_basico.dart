// async/await: ordem entre código síncrono, await e o resultado.
Future<int> dobro(int x) async {
  print('dobro início $x');
  await null;
  print('dobro fim $x');
  return x * 2;
}

Future<void> main() async {
  print('início');
  final f = dobro(2);
  print('depois da chamada');
  final r = await f;
  print('resultado $r');
  print(await dobro(5));
  print('fim');
}
