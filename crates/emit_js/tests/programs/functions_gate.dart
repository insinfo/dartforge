// Without closures, tear-offs or function types the production rti
// leaves out its function kinds; a function type that only a supertype's
// argument names still makes it keep them. Both programs match the VM.

class Holder implements Comparable<int Function()> {
  @override
  int compareTo(int Function() other) => 0;
}

class Box<T> {
  bool test(Object? o) => o is T;
}

void main() {
  final Object h = Holder();
  print([h is Comparable<Object>, h is Comparable<int>, h is Comparable<dynamic>]);
  print(h.runtimeType);
  print(Box<int>().test(1));
  print(Box<String>().test(1));
  print(<int>[1, 2].runtimeType);
  final buffer = StringBuffer()..write('a')..write(1);
  print(buffer);
}
