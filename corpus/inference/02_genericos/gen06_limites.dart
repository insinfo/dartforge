// R-GEN-06: T sem restrição recebe o limite (instantiate-to-bounds);
// o tipo cru `C` também.
class C<T extends num> {}

T f<T extends num>() => throw 0;
void main() {
  var c = /*@*/C();
  C cru = /*@*/C();
  var x = /*@*/f();
  List l = /*@*/[];
  print([c, cru, x, l]);
}
