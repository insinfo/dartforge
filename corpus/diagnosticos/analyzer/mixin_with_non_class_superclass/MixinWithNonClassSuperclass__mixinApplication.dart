int A = 0;
mixin B {}
class C = A with B;
//        ^
// [diag.mixinWithNonClassSuperclass] Mixin can only be applied to class.
