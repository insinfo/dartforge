class C {
  int get id => 0;
  void set id(int v) {}
}

extension Ext on C {
  int get id => 1;
}

f(C c) {
  Ext(c).id++;
//       ^^
// [diag.undefinedExtensionSetter] The setter 'id' isn't defined for the extension 'Ext'.
}
