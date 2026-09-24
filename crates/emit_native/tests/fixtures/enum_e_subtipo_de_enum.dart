enum E {
  a,
  b;
}

class C {}

void main() {
  Object o = E.a;
  print(o is Enum);
  print(o is C);
  print(E.b is Enum);
  print(C() is Enum);
}
