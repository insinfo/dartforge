// A function type that is the target of `is`/`as`: the production profile
// keeps `_isFunctionSubtype`, and every answer stays the VM's.

void main() {
  final Object a = (int x) => x + 1;
  final Object b = (String s) => s.length;
  print(a is int Function(int));
  print(b is int Function(int));
  print(a is Object Function(Never));
  final f = a as int Function(int);
  print(f(41));
  try {
    (b as int Function(int))(1);
  } catch (e) {
    print(e is TypeError);
  }
  final callbacks = <void Function()>[() => print('one'), () => print('two')];
  for (final c in callbacks) {
    c();
  }
}
