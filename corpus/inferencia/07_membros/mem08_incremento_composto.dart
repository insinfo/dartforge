// R-MEM-08: ++/-- prefixo e sufixo, atribuição composta, índice.
void f(int i, double d, num n, List<int> l, Map<String, int> m, List<int>? ln) {
  print([/*@*/i++, /*@*/++i, /*@*/i += 1, /*@*/d += 1, /*@*/n += 1.5]);
  print([/*@*/l[0], /*@*/l[0] += 1, /*@*/l[0]++, /*@*/m['a'], /*@*/m['a'] ??= 2, /*@*/ln?[0]]);
}

void main() => f(1, 2, 3, [1], {}, null);
