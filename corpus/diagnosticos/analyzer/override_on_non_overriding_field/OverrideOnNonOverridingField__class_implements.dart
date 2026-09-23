class A {
  int get a => 0;
}
class B implements A {
  @override
  final int a = 1;
}