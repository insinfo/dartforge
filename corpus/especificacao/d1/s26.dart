class A {
  final int a;
  const A(dynamic d) : a = d.length;
}
const x = const A(<int>[]);
const y = const A('abc');
