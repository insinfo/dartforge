// R-FLU-15: `?.` na condição: `a?.v != null` e a promoção do alvo `a`.
class A {
  int? v;
  int w = 0;
}

void f(A? a, A? b) {
  if (a?.v != null) {
    print(/*@*/a);
  }
  if (b?.w != null) {
    print(/*@*/b);
  }
}

void main() => f(A(), A());
