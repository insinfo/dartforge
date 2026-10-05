// @dart=2.16
enum E<T> with M implements I { a<int>(1), b.x(); const E(int x); const E.x(); }
mixin M {}
class I {}
