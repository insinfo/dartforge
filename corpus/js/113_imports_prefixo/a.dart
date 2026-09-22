// Biblioteca a: função nome(), classe Forma, enum, constante e variável top-level.
String nome() => 'biblioteca a';

class Forma {
  final String tipo;
  Forma(this.tipo);
  String descreve() => 'Forma de a: $tipo';
}

enum Nivel { baixo, medio, alto }

const versao = 'a-1.0';
var contador = 0;

int incrementa() => ++contador;

String usaContador() => 'a vê contador=$contador';
