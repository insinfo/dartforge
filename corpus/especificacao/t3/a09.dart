class A {
  Never get n => throw 0;
  int get i => 0;
}

void f(A a) {
  if (a case A(n: var x, i: var y)) {
    print(x);
  }
  print(2);
}
