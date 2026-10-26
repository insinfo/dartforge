// `super.x` no corpo de um construtor (emitido como função, onde `super` não
// é JavaScript válido): leitura de getter e de campo, escrita no setter da
// superclasse, chamada, tear-off ligado a `this` e uso dentro de uma closure;
// e `super.campo` em métodos.
class Foo {
  bool myBoolean = false;
  void set foo(bool b) {
    print('Foo.foo $b');
    myBoolean = b;
  }

  int get g => 1;
  int m() => 2;
  int operator +(int o) => 100 + o;
}

class Baz extends Foo {
  void set foo(bool b) => print('Baz.foo');
  int get g => 10;
  int m() => 20;
  int operator +(int o) => 0;
  final List<int> seen = [];

  Baz() {
    super.foo = true;
    print([super.myBoolean, super.g, super.m(), (() => super.m())(), super + 1]);
    final t = super.m;
    print([t(), (super.foo = false), super.myBoolean]);
    [1, 2].map((x) => super.m() + x).forEach(seen.add);
    print(seen);
  }
}

void main() {
  Baz();
  fields();
}

// `super.campo` de um campo sem acessores (privado, ou que ninguém
// sobrescreve) e de um campo sobrescrito, lido e escrito num método.
class Fields {
  bool flag = true;
  int count = 1;
  int _x = 3;
}

class SubFields extends Fields {
  int count = 5;
  int read() => super.count;
  bool flagOf() => super.flag;
  int x() => super._x;
  void write() {
    super.count = 9;
    super._x += 1;
  }
}

void fields() {
  final s = SubFields();
  print([s.read(), s.flagOf(), s.count, s.x()]);
  s.write();
  print([s.read(), s.count, s.x()]);
}
