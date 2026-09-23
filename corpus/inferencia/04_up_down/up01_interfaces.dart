// R-UP-01: UP de interfaces: o supertipo comum de maior profundidade única.
class A {}

class B implements A {}

class C implements A {}

class I1 {}

class I2 {}

class X implements I1, I2 {}

class Y implements I1, I2 {}

class Z extends B {}

void main(List<String> args) {
  var b = args.isEmpty;
  var p = /*@*/b ? B() : C();
  var q = /*@*/b ? X() : Y();
  var r = /*@*/b ? B() : Z();
  var s = /*@*/b ? 1 : 2.5;
  var t = /*@*/b ? 1 : 'a';
  print([p, q, r, s, t]);
}
