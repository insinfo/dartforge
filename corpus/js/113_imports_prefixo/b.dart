// Biblioteca b: mesmos nomes de a (nome, Forma, versao, contador) com outros valores.
String nome() => 'biblioteca b';

class Forma {
  final int lados;
  Forma(this.lados);
  String descreve() => 'Forma de b: $lados lados';
}

enum Nivel { um, dois }

const versao = 'b-2.0';
var contador = 100;

typedef Transforma = String Function(String);

Transforma maiuscula = (s) => s.toUpperCase();

String aplica(Transforma t, String s) => t(s);

class Util {
  static int dobra(int x) => x * 2;
  static const pi = 3;
}
