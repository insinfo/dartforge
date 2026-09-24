int A = 0;
mixin B {}
class C extends A with B {}
//              ^
// [diag.mixinWithNonClassSuperclass] Mixin can only be applied to class.
