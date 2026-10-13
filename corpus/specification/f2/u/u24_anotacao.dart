class _Anot { const _Anot(); }
const _c = 1;
const _d = 2;
class _SoAnot { const _SoAnot.n(); const _SoAnot.o(); }
@_Anot()
@_SoAnot.n()
void f([@_c int? x]) {}
