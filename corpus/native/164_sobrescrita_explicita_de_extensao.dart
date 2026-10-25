// A sobrescrita explícita de extensão (`E1(c).g`) e o operador resolvido para
// uma extensão numa atribuição composta (`a.field += b`, `++a.field`) chamam o
// membro da extensão, mesmo quando a classe tem um membro homônimo.
class C {
  int get g => 1;
  set s(int v) => print('C.s $v');
  int m() => 3;
  int operator +(int o) => 100;
  int field = 0;
}

extension E1 on C {
  int get g => 2;
  set s(int v) => print('E1.s $v');
  int m() => 4;
  int operator +(int o) => 200;
  int get field => 7;
  set field(int v) => print('E1.field= $v');
}

class Box {
  Object field = 'box';
}

extension Ops on Object {
  Object operator +(Object other) => '(${this}+$other)';
  Object operator -(Object other) => '(${this}-$other)';
}

void main() {
  final c = C();
  print([E1(c).g, E1(c).m(), E1(c) + 1, E1(c).field, c.g, c.m(), c.field]);
  E1(c).s = 5;
  E1(c).field = 9;
  E1(c).field += 1;
  final t = E1(c).m;
  print(t());

  final b = Box();
  print(b.field += 'x');
  print(b.field -= 'y');
  print(++b.field);
  print(b.field++);
  print(b.field);
}
