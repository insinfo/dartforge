class A {
  const A(int x) : assert(x >= 0);
}

main() {
  var c = const A(int.fromEnvironment('x'));
  print(c);
}
