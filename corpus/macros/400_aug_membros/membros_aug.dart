augment library 'main.dart';

import 'dart:math' as m;

augment class Conta {
  final List<int> historico = [];

  augment void depositar(int valor) {
    _saldo += valor;
    historico.add(valor);
  }

  augment int get saldo => _saldo;

  augment set limite(int v) {
    print('limite ${m.max(v, 1)}');
  }

  augment Conta operator +(int valor) {
    depositar(valor);
    return this;
  }

  int dobro() => _saldo * 2;

  String descrever() => 'Conta de $dono com ${historico.length} depósitos';
}

augment String saudacao(String nome) => 'olá, $nome';
