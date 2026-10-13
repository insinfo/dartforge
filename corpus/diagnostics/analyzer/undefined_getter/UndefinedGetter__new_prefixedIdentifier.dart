class C {}

abstract class D {
  C get c;
}

f(D d) {
  d.c.new;
//    ^^^
// [diag.undefinedGetter] The getter 'new' isn't defined for the type 'C'.
}
