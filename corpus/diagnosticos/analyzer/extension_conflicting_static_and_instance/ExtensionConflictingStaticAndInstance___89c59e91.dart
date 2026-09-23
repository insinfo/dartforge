class A {
  static set foo(_) {}
  set bar(_) {}
}

extension E on A {
  set foo(_) {}
  static set bar(_) {}
}
