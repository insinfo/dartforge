// Convertido de tests/conformance/cases/class_effects.dart (fixture antigo do corpus de conformidade).
int mark(int value) { print(value); return value; }
class Parent {
  int first = mark(1);
  int value() { return this.first; }
}
class Child extends Parent {
  final int second = mark(2);
  int value() { return this.first + this.second; }
}
Child receiver() { print(3); return Child(); }
void main() {
  var value = Child();
  print(value.value());
  receiver().first = mark(4);
  print(value == value);
  print(Child() == Child());
}
