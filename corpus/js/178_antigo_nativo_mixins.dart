// Convertido de tests/native/cases/mixins.dart (fixture antigo do backend nativo).
// Oracle Dart VM 3.6.2: inicializadores derivados primeiro, campos e despacho por aplicação.
int mark(int value) { print(value); return value; }
class Parent {
  int prefix = mark(7);
  int who() { return 1; }
}
mixin Counter {
  int count = mark(10);
  int inc() { count = count + 1; return count; }
  int invoke() { return inc(); }
  int get current => count;
  int who() { return 2; }
}
mixin Last {
  int last = mark(20);
  int who() { return 3; }
}
class Combined extends Parent with Counter, Last {
  int own = mark(30);
}
class OtherParent {
  int first = mark(100);
  int second = mark(200);
}
class Other extends OtherParent with Counter {}
mixin class Independent {
  int value = 5;
  int readValue() { return value; }
}
class AppliedIndependent with Independent {}
void main() {
  Combined combined = Combined();
  print(combined.prefix);
  print(combined.inc());
  print(combined.invoke());
  print(combined.who());
  print(combined.current);
  Counter counter = combined;
  print(counter.inc());
  Other other = Other();
  print(other.inc());
  print(other.first);
  print(other.second);
  print(combined.current);
  print(Independent().readValue());
  print(AppliedIndependent().readValue());
}
