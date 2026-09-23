// R-PAD-06: padrão em for-in e em switch de instrução com registros.
void f(List<(int, String)> l, Map<String, int> m) {
  for (var (i, s) in l) {
    print([/*@*/i, /*@*/s]);
  }
  for (var MapEntry(:key, :value) in m.entries) {
    print([/*@*/key, /*@*/value]);
  }
}

void main() => f([], {});
