class G<T> {}
class H<T> {
  T<Unresolved>? a;
  T<int>? b;
}
G<Unresolved>? v1;
G<Unresolved, int>? v2;
Unresolved<Other>? v3;
G<G<Unresolved>>? v4;
dynamic<Unresolved>? v5;
int topv = 0;
topv<int>? v6;
void f<X>() {
  f<Unresolved>();
  X<Unresolved> l;
  int loc = 0;
  G<loc>? m;
  G<f>? n;
  G<topv>? o;
}
class S1 extends G<Unresolved> {}
class S2 implements G<topv> {}
