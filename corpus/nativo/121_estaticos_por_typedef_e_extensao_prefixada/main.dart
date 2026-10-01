// Membro estático pelo nome de um `typedef` (especificação §15 "Type
// Aliases": `A.x` é o membro estático `x` da classe que o alias denota) e
// aplicação explícita de extensão importada com prefixo (`p.E(x).m`,
// §13.3). As duas viravam "não suportado: elemento de topo" (o
// `PostgreSQLDataType.text` do postgres_fork e o
// `async.StreamSinkExtensions(socket)` do postgres; docs/NATIVO-PROJETOS-REAIS.md).

import 'tipos.dart';
import 'tipos.dart' as t;

String nomeDe(Apelido? c) {
  switch (c) {
    case Apelido.texto:
      return 'text';
    case t.Apelido.inteiro:
      return 'int';
    default:
      return '?';
  }
}

void main() {
  print(nomeDe(Apelido.texto));
  print(nomeDe(t.Apelido.inteiro));
  print(nomeDe(Apelido.fabricar('x')));
  print(Apelido.fabricar('x'));
  Apelido.contador++;
  t.Apelido.contador += 2;
  ApelidoGenerico.contador *= 5;
  print(Apelido.contador);
  print(Apelido.dobroDoContador);
  print(t.ApelidoGenerico.dobroDoContador);
  print(ApelidoDeEnum.values);
  print(t.ApelidoDeEnum.verde.index);
  final f = Apelido.fabricar;
  print(f('y'));
  final g = t.Apelido.fabricar;
  print(g('z'));
  print(t.Dobro(21).dobro);
  print(t.Dobro(3).vezes(5));
  print(Dobro(4).dobro);
  final v = t.Dobro(6).vezes;
  print(v(7));
}
