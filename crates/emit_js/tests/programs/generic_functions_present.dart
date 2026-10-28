// A generic function type: the production profile keeps the substitution of
// generic function types, and every type and instantiation stays the VM's.

T identity<T>(T x) => x;

List<T> twice<T extends Object>(T x) => [x, x];

void main() {
  print(identity<double>(1.5));
  final int Function(int) id = identity;
  print(id(4));
  print(id.runtimeType);
  print(twice<String>('a'));
  final Object g = twice;
  print(g is Function);
  print(g is List<int> Function(int));
}
