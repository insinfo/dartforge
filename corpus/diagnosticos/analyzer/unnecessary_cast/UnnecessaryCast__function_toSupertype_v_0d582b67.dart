void f(void Function(num) a) {
  (a as void Function(int))(3);
}
