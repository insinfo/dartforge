class A {
  bool operator ==(Object other) => false;
}

enum E implements A {
  v;
}
