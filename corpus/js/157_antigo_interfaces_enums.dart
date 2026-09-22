// Convertido de tests/conformance/cases/interfaces_enums.dart (fixture antigo do corpus de conformidade).
// Programa original; oracle Dart VM 3.6.2, incluindo redeclaracao abstrata herdada.
enum Color { red, green, blue }
enum Signal { red, green }
abstract class Named {
  String name();
  int? measure(int value);
}
abstract class NamedAgain implements Named {}
class Parent {
  int inherited() { return 17; }
}
abstract class Middle extends Parent {
  int inherited();
}
class Leaf extends Middle implements NamedAgain {
  String name() { return 'leaf'; }
  int measure(int? value) { return value ?? 99; }
}
String describe(Named item) { return item.name(); }
Color echo(Color color) { return color; }
Color? absent() { return null; }
abstract class Action { void run(); }
class ConcreteAction implements Action {
  int run() { print(81); return 82; }
}
void main() {
  Action action = ConcreteAction();
  action.run();
  Leaf leaf = Leaf();
  Named named = leaf;
  NamedAgain again = leaf;
  Middle middle = leaf;
  print(named.name());
  print(named.measure(3));
  print(describe(again));
  print(middle.inherited());
  print(echo(Color.green).name);
  print(Color.blue.index);
  print(Color.red == echo(Color.red));
  print(Color.red != Color.green);
  print(Color.red == Signal.red);
  Color? color = absent();
  print(color == null);
  print((color ?? Color.green).name);
  color = Color.blue;
  print(color!.index);
  for (var i = 0; i < 50; i++) {
    String churn = 'a' + 'b';
    if (i == 49) { print(churn); print(echo(Color.red).name); }
  }
}
