class S {
  final int f;
  S(this.f);
}

class C extends S {
  C() : this.other(super.f);
//                 ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
  C.other(int a) : super(a);
}
