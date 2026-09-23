// requer-dart: 3.13
// Parâmetro declarante sem tipo (spec :945-962): o tipo do getter herdado de
// mesmo nome; senão o do default; senão `Object?`.
abstract class TemNome {
  String get nome;
}

class Pessoa(final nome) implements TemNome {
  String saudacao() => 'olá, ${nome.toUpperCase()}';
}

class Config([var limite = 10, var rotulo]);

void main() {
  print(Pessoa('ana').saudacao());
  var c = Config();
  print('${c.limite + 1} ${c.rotulo}');
  c.rotulo = 3;
  print(c.rotulo);
  c.rotulo = 'texto';
  print(c.rotulo);
  print(Config.new.runtimeType);
}
