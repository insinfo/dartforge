// Part 1 da biblioteca main.dart: classe Conta com campo privado, enum com campo privado, constante.
part of 'main.dart';

const constanteP1 = 10;

String saudacaoP1(String s) => _formata('olá $s ${_privadaP2()}');

String _privadaP1() => 'privada de p1 vê segredo=$_segredoDoMain';

void incrementaSegredo() => _segredoDoMain++;

class Conta {
  final String dono;
  int _saldo;
  final List<int> _historico = [];
  Conta(this.dono, this._saldo);

  int get saldo => _saldo;

  void deposita(int v) {
    _saldo += v;
    _historico.add(v);
  }

  @override
  String toString() => 'Conta($dono, $_saldo)';
}

enum Cor {
  vermelho('f00'),
  verde('0f0');

  final String _codigo;
  const Cor(this._codigo);
  String get hex => '#$_codigo';
}
