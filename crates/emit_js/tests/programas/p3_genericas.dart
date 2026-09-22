class Box<T> {
  T v;
  Box(this.v);
  List<T> wrap() => <T>[v];
  bool same<S>(S x) => x is T && v is S;
  Map<String, T> toMap() => {'v': v};
  static Box<E> of<E>(E e) => Box<E>(e);
  Box<S> map<S>(S Function(T) f) => Box<S>(f(v));
}
class Pair<A, B> extends Box<A> {
  B b;
  Pair(A a, this.b) : super(a);
  String show() => '$v,$b';
}
List<T> dup<T>(T x) => <T>[x, x];
List<List<T>> nest<T extends num>(T x) => [dup(x)];
T? firstOr<T>(List<T> l, [T? d]) => l.isEmpty ? d : l[0];
void main() {
  var b = Box<int>(1);
  print(b.wrap());
  print(b.same<int>(2));
  print(b.same<String>('s'));
  print(b.toMap());
  print(Box.of('x').v);
  print(b.map((x) => x.toString()).v);
  var p = Pair<int, String>(1, 'a');
  print(p.show());
  print(p is Box<int>);
  print(p is Box<num>);
  print(p is Box<String>);
  print(dup(3));
  print(nest(2.5));
  print(firstOr([1, 2]));
  print(firstOr(<int>[], 9));
  print(firstOr(<String>[]));
  var f = dup<int>;
  print(f(4));
  Object o = b;
  print(o is Box<int>);
  print((o as Box).v);
  print(b.runtimeType);
  print(p.runtimeType);
  var l = <Box<int>>[];
  l.add(b);
  print(l.length);
  print(<int>[].runtimeType);
  print((<T>(T x) => x)<int>(3));
  List<int> Function(int) g = dup;
  print(g(5));
}
