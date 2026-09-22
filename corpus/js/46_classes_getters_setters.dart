// Getters e setters: pares, computados, validação, campo virtual, x++ via setter, override de campo por getter.
class Temperatura {
  double _celsius = 0;

  double get celsius => _celsius;
  set celsius(double v) {
    print('set celsius ${v.toStringAsFixed(1)}');
    _celsius = v;
  }

  double get fahrenheit => _celsius * 9 / 5 + 32;
  set fahrenheit(double f) {
    _celsius = (f - 32) * 5 / 9;
  }
}

class Conta {
  int _saldo = 0;
  int _rejeitados = 0;

  int get saldo => _saldo;
  set saldo(int v) {
    if (v < 0) {
      _rejeitados++;
      print('rejeitado $v');
      return;
    }
    _saldo = v;
  }

  int get rejeitados => _rejeitados;
}

class Retangulo {
  int largura;
  int altura;
  Retangulo(this.largura, this.altura);

  int get area => largura * altura;
  int get perimetro => 2 * (largura + altura);
  bool get quadrado => largura == altura;

  set lado(int v) {
    largura = v;
    altura = v;
  }
}

class Base {
  int valor = 10;
  String nome = 'base';
}

class Derivada extends Base {
  @override
  int get valor => 99;

  @override
  String get nome => 'derivada:' + super.nome;
}

class Contador {
  int _n = 0;
  int leituras = 0;
  int escritas = 0;

  int get n {
    leituras++;
    return _n;
  }

  set n(int v) {
    escritas++;
    _n = v;
  }
}

class Estatico {
  static int _x = 1;
  static int get x => _x;
  static set x(int v) => _x = v * 2;
}

void main() {
  final t = Temperatura();
  t.celsius = 25;
  print(t.celsius.toStringAsFixed(1));
  print(t.fahrenheit.toStringAsFixed(1));
  t.fahrenheit = 212;
  print(t.celsius.toStringAsFixed(1));
  t.celsius = 37.5;
  print(t.fahrenheit.toStringAsFixed(2));

  final c = Conta();
  c.saldo = 100;
  print(c.saldo);
  c.saldo = -5;
  print(c.saldo);
  c.saldo -= 30;
  print(c.saldo);
  c.saldo -= 300;
  print(c.saldo);
  print(c.rejeitados);

  final r = Retangulo(3, 4);
  print(r.area);
  print(r.perimetro);
  print(r.quadrado);
  r.lado = 5;
  print(r.area);
  print(r.quadrado);
  r.largura++;
  print(r.largura);
  print(r.quadrado);

  final d = Derivada();
  print(d.valor);
  print(d.nome);
  d.valor = 5;
  print(d.valor);
  final Base b = d;
  print(b.valor);
  print(b.nome);

  final k = Contador();
  k.n = 5;
  k.n++;
  k.n += 10;
  print(k.n);
  print(k.leituras);
  print(k.escritas);
  final antigo = k.n++;
  print(antigo);
  print(k.n);
  print(k.leituras);
  print(k.escritas);

  print(Estatico.x);
  Estatico.x = 5;
  print(Estatico.x);
  Estatico.x++;
  print(Estatico.x);
}
