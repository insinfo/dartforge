// Campos e métodos chamados `constructor`/`prototype`: nomes reservados do
// JavaScript. Em `class`, `get constructor()` é erro de sintaxe, então o DDC
// renomeia para `_constructor`/`_prototype` (memberNameForDartMember,
// compiler/js_names.dart:411). `package:json_annotation` tem um campo
// `constructor`, então isto aparece em código real.
class Anotacao {
  final String constructor;
  final int prototype;
  String get valueOf => 'vo';
  Anotacao(this.constructor, this.prototype);
  String descrever() => '$constructor/$prototype';
}

class Herdeira extends Anotacao {
  Herdeira() : super('c', 7);
  @override
  String descrever() => 'sub:${super.descrever()}';
}

class Mutavel {
  String constructor = 'inicial';
  int prototype = 0;
  void trocar(String c, int p) {
    constructor = c;
    prototype = p;
  }
}

void main() {
  final a = Anotacao('Pessoa', 3);
  print(a.constructor);
  print(a.prototype);
  print(a.descrever());
  print(a.valueOf);
  print(Herdeira().descrever());
  final m = Mutavel()..trocar('outro', 9);
  print('${m.constructor} ${m.prototype}');
  final dinamico = a as dynamic;
  print(dinamico.constructor);
  print(dinamico.descrever());
  print(<String>[a.constructor, m.constructor].join(','));
}
