enum E { a, b }
enum E { c }
void f(E e) {
  E.a;
  E.c;
  E.values;
  switch (e) {
    case E.a:
    case E.b:
  }
}
