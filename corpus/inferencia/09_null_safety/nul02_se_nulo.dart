// R-NUL-02: `??` e `??=`: UP(NonNull(T1), T2).
void f(int? a, double b, String? s, num? n) {
  print([/*@*/a ?? b, /*@*/a ?? null, /*@*/s ?? 'x', /*@*/n ??= 1, /*@*/a ?? a]);
}

void main() => f(1, 2, '', 3);
