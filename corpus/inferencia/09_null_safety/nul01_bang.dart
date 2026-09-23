// R-NUL-01: `e!` tem tipo NonNull(T).
void f<T, U extends Object?>(int? x, List<int>? l, T? t, T t2, U u) {
  print([/*@*/x!, /*@*/l!.first, /*@*/t!, /*@*/t2!, /*@*/u!]);
}

void main() => f<int, int>(1, [1], 1, 1, 1);
