class S {}
mixin M {}
class _Alias = S with M;
class _Base {}
class _Derivada extends _Base {}
void _a() {}
void _b() { _a(); }
