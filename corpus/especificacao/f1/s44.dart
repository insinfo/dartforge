typedef T = int;
extension Ext on int {}
mixin Mx {}
enum En { a }
void fn() {}
void f(Object o) {
  new T();
  new Ext();
  new Mx();
  new En();
  new dynamic();
  new Never();
  new void();
  const fn();
  Ext x1;
  o is Ext;
  <Ext>[];
}
