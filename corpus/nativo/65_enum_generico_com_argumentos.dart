// Valores de enum genérico guardam o tipo deles: os argumentos escritos
// (`bool<core.bool>()`, o `DriftSqlType` do drift) ou, sem eles, os limites
// dos parâmetros. Antes todo valor era `E<dynamic>`, e o `this as
// DriftSqlType<Object>` do `sqlTypeName` falhava no gerador do drift.
import 'dart:core' as core;
import 'dart:core';

sealed class Base<T> {
  String nome();
}

enum Tipo<T extends Object> implements Base<T> {
  bool<core.bool>(),
  string<String>(),
  int<core.int>(),
  lista<List<core.int>>(),
  semArgumentos(),
  nomeado<core.double>.x(2);

  const Tipo();
  const Tipo.x(core.int y);

  @override
  String nome() {
    switch (this as Tipo<Object>) {
      case Tipo.bool:
        return 'b';
      case Tipo.string:
        return 's';
      default:
        return 'o';
    }
  }
}

enum Livre<T> { a, b<core.int>() }

void main() {
  for (final t in Tipo.values) {
    print('${t.runtimeType} ${t.nome()} ${t is Tipo<core.int>} ${t is Base<Object>}');
  }
  print(Tipo.values.byName('string').runtimeType);
  final m = <core.Type, Tipo>{core.int: Tipo.int};
  print(m[core.int].runtimeType);
  for (final l in Livre.values) {
    print(l.runtimeType);
  }
}
