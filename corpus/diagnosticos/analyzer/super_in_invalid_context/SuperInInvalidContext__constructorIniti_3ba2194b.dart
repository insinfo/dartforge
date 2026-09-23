class S {
  final int f;
  S(this.f);
}

class C extends S {
  C() : super(super.f);
//            ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}
