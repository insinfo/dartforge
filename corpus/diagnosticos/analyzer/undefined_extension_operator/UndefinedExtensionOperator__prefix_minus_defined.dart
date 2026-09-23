extension E on String {
  String operator -() => substring(1);
}
f() {
  -E('a');
}
