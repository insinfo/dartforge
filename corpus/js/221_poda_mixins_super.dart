// Poda do perfil de produção (docs/JS-PRODUCAO.md §1.7): mixins, `super` em
// cadeia, aplicação `class C = S with M`, classe abstrata sem construtor no
// meio da cadeia e membro de mixin chamado só por `dynamic`. Instanciar a
// subclasse instancia a cadeia; os membros vivem pelo seletor. Se a poda
// errar, o caso imprime "caso <nome>: ERRO" ou um nome de classe trocado.

void caso(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('caso $nome: ERRO $e');
  }
}

class Raiz {
  final String id;
  Raiz(this.id);
  String descreve() => 'Raiz($id)';
}

abstract class Meio extends Raiz {
  Meio() : super('meio');
  String extra() => 'Meio.extra';
}

mixin Registro on Raiz {
  final List<String> eventos = [];
  String descreve() => 'Registro+${super.descreve()}';
  void registra(String e) => eventos.add(e);
  String soDinamico() => 'Registro.soDinamico';
}

mixin Contador {
  int n = 0;
  int incrementa() => ++n;
}

class Folha extends Meio with Registro, Contador {
  @override
  String descreve() => 'Folha+${super.descreve()}';
}

class Base {
  String quem() => 'Base';
}

mixin Eco {
  String eco(String s) => '$s $s';
}

class Aplicada = Base with Eco;

void main() {
  final f = Folha();
  caso('super em cadeia', () => f.descreve());
  caso('mixin com estado', () {
    f.registra('a');
    f.incrementa();
    return '${f.eventos} ${f.incrementa()}';
  });
  caso('herdado de abstrata', () => f.extra());
  caso('mixin só por dynamic', () {
    dynamic d = f;
    return d.soDinamico();
  });
  caso('aplicação de mixin', () {
    final a = Aplicada();
    return '${a.quem()} ${a.eco('oi')} ${a.runtimeType} ${a is Aplicada}';
  });
}
