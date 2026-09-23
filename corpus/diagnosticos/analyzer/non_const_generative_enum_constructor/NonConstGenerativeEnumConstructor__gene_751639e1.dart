// %before-language-feature: primary-constructors
enum E {
  v;
  E();
//^
// [diag.nonConstGenerativeEnumConstructor] Generative enum constructors must be 'const'.
}
