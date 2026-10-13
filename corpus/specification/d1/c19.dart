int g() => 1;
var v = 1;
void f([int a = v, int b = g(), c = undefinedName, d = const [v]]) {}
class A {
  const A([this.x = v]);
  final int x;
}
