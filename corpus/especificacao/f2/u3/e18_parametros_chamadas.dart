class _C {
  _C([int? a]);
  _C.r() : this(1);
  _C.s([int? b]);
  _C.t() : this.s(2);
  const _C.k([int? c]);
}
class _D extends _C {
  _D() : super.s(3);
}
void _f([int? a]) {}
void _g([int? a]) {}
void _h([int? a]) {}
class _An { const _An([int? x]); }
@_An(1)
void main() {
  _C.r(); _C.t(); _D();
  (_f)(1);
  _g.call(1);
  var h = _h;
  h(1);
  void _local([int? a]) {}
  void local([int? a]) {}
  _local(); local();
  void cb(void Function([int? z]) p) {}
  cb(([z]) {});
}
