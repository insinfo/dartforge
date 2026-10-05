class A {
  final int x;
  const A({required this.x});
}
const a = const A(x: 'a' as dynamic);
const b = const A(x: 1 ~/ 0);
const c = const A(x: v);
var v = 1;
