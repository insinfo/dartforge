// R-FLU-13: promoção de variável de tipo cria interseção `T & S`.
void f<T, U extends num?>(T t, U u) {
  if (t is int) {
    print(/*@*/t);
    print(/*@*/t.isEven);
  }
  if (u != null) print(/*@*/u);
  if (u is int) print(/*@*/u);
  if (t != null) print(/*@*/t);
}

void main() => f(1, 2);
