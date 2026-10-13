extension type E(int i) { const E.c(this.i); int get twice => i * 2; }
const a = E.c(1);
const b = a.twice;
const c = a + 1;
const d = const E.c('x' as dynamic);
