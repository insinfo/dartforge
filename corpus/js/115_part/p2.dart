// Part 2 da biblioteca main.dart: classe Relatorio usando Conta._saldo de p1, top-level privado, função usando import do main.
part of 'main.dart';

const constanteP2 = 20;

int _contadorP2 = 0;
final registrados = <String>[];

String _privadaP2() => 'privada de p2';

void registra(String s) {
  _contadorP2++;
  registrados.add(_formata(s));
}

_Escondida criaEscondida(int v) => _Escondida(v * 2);

int dobraSegredo() => _segredoDoMain * 2;

class Relatorio {
  final List<Conta> contas;
  Relatorio(this.contas);

  int total() => contas.fold(0, (a, c) => a + c._saldo);

  // max vem do import de dart:math feito em main.dart: parts compartilham os imports.
  int maior() => contas.fold(0, (a, c) => max(a, c._saldo));

  String _descricao() => contas.map((c) => '${c.dono}:${c._saldo}').join(' ');

  String usaDeMain() => _formata(_Escondida(total()).toString());
}
