// part/part of: uma biblioteca em três arquivos; parts definem classes, funções e top-level e acessam membros privados uns dos outros.
import 'dart:math' show max;

part 'p1.dart';
part 'p2.dart';

int _segredoDoMain = 7;

String _formata(String s) => '[$s]';

class _Escondida {
  final int v;
  _Escondida(this.v);
  @override
  String toString() => '_Escondida($v)';
}

void main() {
  print(_formata('main'));
  print(saudacaoP1('mundo'));
  print(_privadaP1());
  print(_privadaP2());
  print(_segredoDoMain);
  incrementaSegredo();
  print(_segredoDoMain);
  print('--');
  final c = Conta('ana', 10);
  c.deposita(5);
  print(c);
  print(c._saldo);
  c._saldo = 100;
  print(c.saldo);
  print(c._historico);
  print('--');
  final r = Relatorio([Conta('a', 1), Conta('b', 3), Conta('c', 2)]);
  print(r.total());
  print(r.maior());
  print(r._descricao());
  print(r.usaDeMain());
  print('--');
  print(_Escondida(1));
  print(criaEscondida(5));
  print(_contadorP2);
  registra('x');
  registra('y');
  print(_contadorP2);
  print(registrados);
  print(constanteP1);
  print(constanteP2 + constanteP1);
  print(Cor.verde.hex);
  print(Cor.values.map((c) => c._codigo).toList());
  print(max(constanteP1, constanteP2));
  print(dobraSegredo());
  print('fim');
}
