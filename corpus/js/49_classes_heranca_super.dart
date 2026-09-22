// Herança: extends, override com super.m(), campos herdados, super em getters, super.nomeado, polimorfismo.
class Animal {
  final String nome;
  int energia = 10;
  Animal(this.nome);
  Animal.cansado(this.nome) : energia = 1;

  String som() => '...';

  String apresenta() => '$nome faz ${som()}';

  void come(int qtd) {
    energia += qtd;
  }

  String get descricao => 'Animal $nome';

  @override
  String toString() => 'Animal($nome, $energia)';
}

class Cachorro extends Animal {
  Cachorro(String nome) : super(nome);
  Cachorro.cansado(String nome) : super.cansado(nome);

  @override
  String som() => 'au';

  @override
  void come(int qtd) {
    super.come(qtd * 2);
    print('$nome comeu');
  }

  @override
  String get descricao => '${super.descricao} (cachorro)';
}

class Filhote extends Cachorro {
  Filhote(String nome) : super(nome);

  @override
  String som() => super.som() + ' ' + super.som();

  @override
  String get descricao => '${super.descricao} filhote';

  @override
  String toString() => 'Filhote:' + super.toString();
}

class Gato extends Animal {
  Gato(super.nome);
  @override
  String som() => 'miau';
}

class Conta {
  int saldo;
  Conta(this.saldo);
  void saca(int v) {
    saldo -= v;
  }
}

class ContaEspecial extends Conta {
  final int limite;
  ContaEspecial(super.saldo, this.limite);
  @override
  void saca(int v) {
    if (saldo + limite >= v) {
      super.saca(v);
    } else {
      print('sem limite para $v');
    }
  }
}

void main() {
  final a = Animal('bicho');
  final c = Cachorro('rex');
  final f = Filhote('bob');
  final g = Gato('tom');
  print(a.apresenta());
  print(c.apresenta());
  print(f.apresenta());
  print(g.apresenta());
  print(a.descricao);
  print(c.descricao);
  print(f.descricao);
  print(g.descricao);

  print(c);
  c.come(3);
  print(c.energia);
  print(c);
  a.come(3);
  print(a.energia);
  f.come(1);
  print(f);
  print(Cachorro.cansado('zé'));
  print(g);

  final List<Animal> todos = [a, c, f, g];
  for (final x in todos) {
    print('${x.nome}: ${x.som()}');
  }
  print(todos.map((x) => x.runtimeType).join(','));
  print(todos.whereType<Cachorro>().length);
  print(todos.where((x) => x is Cachorro).map((x) => x.nome).toList());
  print(f is Cachorro);
  print(f is Animal);
  print(c is Filhote);

  final ce = ContaEspecial(10, 5);
  ce.saca(12);
  print(ce.saldo);
  ce.saca(10);
  print(ce.saldo);
  final Conta cc = ce;
  cc.saca(3);
  print(cc.saldo);
  print(cc is ContaEspecial);
}
