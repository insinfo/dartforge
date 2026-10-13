// R-UP-08: inference-update-3 (3.4): se UP(T1, T2) não é subtipo do
// contexto K mas T1 <: K e T2 <: K, o tipo da expressão é K.
class A {}

class B1 implements A {}

class B2 implements A {}

class C1 implements B1, B2 {}

class C2 implements B1, B2 {}

void main(List<String> args) {
  var b = args.isEmpty;
  var sem = /*@*/b ? C1() : C2();
  B1 com = /*@*/b ? C1() : C2();
  C1? n;
  B2 seNulo = /*@*/n ?? C2();
  B1 sw = /*@*/switch (b) { true => C1(), false => C2() };
  print([sem, com, seNulo, sw]);
}
