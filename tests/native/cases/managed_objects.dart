// Programa original; resultado conferido com Dart VM 3.6.2.
String mark(String text) { print(text); return text; }
class Base {
  String label = mark('base');
  int? value = null;
  String describe() { return this.label; }
  int? adapt(int n) { return n; }
}
class Child extends Base {
  String child = mark('child');
  String describe() { return this.child + ':' + this.label; }
  int adapt(int? n) { return n ?? 99; }
}
class Node {
  Node? next = null;
  String text = '';
  bool? flag = null;
}
String join(String first, String second) { return first + second; }
String fresh(String prefix) { return prefix + '!'; }
Base make() { return Child(); }
void cycle() {
  Node a = Node();
  Node b = Node();
  a.next = b;
  b.next = a;
  a.text = fresh('A');
  b.text = fresh('B');
  a.flag = false;
  print(a.next!.next == a);
  print(a.next!.text);
  print(a.flag);
  a.flag = null;
  print(a.flag);
}
void main() {
  Base object = make();
  print(object.describe());
  print(object.adapt(7));
  object.value = 42;
  print(object.value);
  object.value = null;
  print(object.value);
  print(join(fresh('left'), fresh('right')));
  String? optional = null;
  print(optional);
  print(optional ?? fresh('fallback'));
  optional = fresh('same');
  print(optional == fresh('same'));
  print(optional! + '\u{1F600}');
  print('a\u0000b' == 'a' + '\u0000b');
  for (var i = 0; i < 3; i++) { cycle(); }
}
