int soma(int n1, int n2) => n1 + n2;
int soma2(int n1, int n2) => n1 + n2;
void discard() => 42;
void effect() => print(soma(20, 22));
class Value {
  int number() => 7;
}
void main() {
  discard();
  effect();
  print(soma2(1, 2));
  print(Value().number());
}
