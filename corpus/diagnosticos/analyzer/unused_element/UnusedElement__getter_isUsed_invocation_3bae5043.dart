class A {
  int __a = 0;
  int get _a => __a;
  void set _a(int val) {
    __a = val;
  }
  int b() => _a++;
}
class B extends A {
  @override
  int get _a => 3;
}
