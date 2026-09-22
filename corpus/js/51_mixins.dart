// Mixins: on, with múltiplos e linearização (super.m em cadeia), campos, mixin class, genérico, class C = A with M.
class Base {
  String nome() => 'Base';
  void log(List<String> acc) {
    acc.add('Base');
  }
}

mixin Cor on Base {
  @override
  String nome() => 'Cor(' + super.nome() + ')';
  @override
  void log(List<String> acc) {
    acc.add('Cor-antes');
    super.log(acc);
    acc.add('Cor-depois');
  }
}

mixin Tamanho on Base {
  int tamanho = 1;
  @override
  String nome() => 'Tamanho(' + super.nome() + ')';
  @override
  void log(List<String> acc) {
    acc.add('Tamanho-antes');
    super.log(acc);
    acc.add('Tamanho-depois');
  }
}

class CorTamanho extends Base with Cor, Tamanho {}

class TamanhoCor extends Base with Tamanho, Cor {
  @override
  String nome() => 'TC:' + super.nome();
}

mixin Contador {
  int _contagem = 0;
  int get contagem => _contagem;
  void conta() {
    _contagem++;
  }
}

mixin Saudacao {
  String get quem;
  String saude() => 'ola, $quem';
}

class Visitante with Contador, Saudacao {
  @override
  final String quem;
  Visitante(this.quem);
}

mixin class Util {
  int dobra(int x) => x * 2;
}

class UsaUtil with Util {}

class HerdaUtil extends Util {}

mixin Pilha<T> {
  final List<T> _itens = [];
  void empilha(T t) => _itens.add(t);
  T desempilha() => _itens.removeLast();
  int get tamanho => _itens.length;
  List<T> get itens => List.unmodifiable(_itens);
}

class PilhaInt with Pilha<int> {}

class PilhaStr with Pilha<String> {
  String junta() => _itens.join('+');
}

class Simples extends Base with Cor {}

class Aplicada = Base with Cor, Tamanho;

void main() {
  final ct = CorTamanho();
  print(ct.nome());
  final acc1 = <String>[];
  ct.log(acc1);
  print(acc1.join(' > '));

  final tc = TamanhoCor();
  print(tc.nome());
  final acc2 = <String>[];
  tc.log(acc2);
  print(acc2.join(' > '));
  print(Simples().nome());

  print(ct is Cor);
  print(ct is Tamanho);
  print(ct is Base);
  print(Simples() is Tamanho);
  ct.tamanho = 5;
  print(ct.tamanho);

  final v = Visitante('ana');
  v.conta();
  v.conta();
  v.conta();
  print(v.contagem);
  print(v.saude());
  print(v is Contador);
  print(v is Saudacao);

  print(UsaUtil().dobra(4));
  print(HerdaUtil().dobra(5));
  print(Util().dobra(6));
  print(UsaUtil() is Util);

  final pi = PilhaInt();
  pi.empilha(1);
  pi.empilha(2);
  pi.empilha(3);
  print(pi.desempilha());
  print(pi.tamanho);
  print(pi.itens);
  final ps = PilhaStr();
  ps.empilha('a');
  ps.empilha('b');
  print(ps.junta());
  print(pi is Pilha<int>);
  print(pi is Pilha<String>);
  print(ps is Pilha<String>);

  final ap = Aplicada();
  print(ap.nome());
  final acc3 = <String>[];
  ap.log(acc3);
  print(acc3.length);
  print(ap is Cor && ap is Tamanho);
  print(ap.tamanho);
}
