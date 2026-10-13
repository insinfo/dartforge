sealed class S {}

class A extends S {}

class B extends S {}

void f(S s) {
  switch (s) {
    case A():
      print(1);
    case B():
      print(2);
    case S():
      print(3);
    case _:
      print(4);
  }
  print(5);
}

int g(S s) {
  switch (s) {
    case A():
      return 1;
    case B():
      return 2;
  }
  print(6);
}
