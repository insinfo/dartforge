// Escopo léxico de identificadores: membro herdado não está no escopo
// léxico da subclasse; a declaração de topo homônima vence o `this`
// implícito. Membro declarado na própria classe vence a de topo.
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
