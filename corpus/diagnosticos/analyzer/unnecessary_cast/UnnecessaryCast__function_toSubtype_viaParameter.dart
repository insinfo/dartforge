void f(void Function(int) a) {
  (a as void Function(num))(3);
}
