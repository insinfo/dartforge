// Lexical scope of identifiers: an inherited member is not in the subclass's
// lexical scope; the top-level declaration with the same name wins over the
// implicit `this`. A member declared in the class itself wins over the top-level one.
String nome = 'topo';

String saudacao() => 'saudação de topo';

class Base {
  String get nome => 'herdado';
  String saudacao() => 'saudação herdada';
}

class Filha extends Base {
  String mostra() => '$nome / ${saudacao()} / ${this.nome} / ${this.saudacao()}';
}

class Propria {
  String get nome => 'própria';
  String mostra() => nome;
}

void main() {
  print(Filha().mostra());
  print(Propria().mostra());
  nome = 'topo alterado';
  print(Filha().mostra());
}
