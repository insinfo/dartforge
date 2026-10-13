void _f([int? a]) {}
class _C([int? a, this.b]) {
  int? b;
  _C.n({int? c});
}
enum E([int? x]) { a }
void main() {
  _f();
  _C();
  _C.n();
}
