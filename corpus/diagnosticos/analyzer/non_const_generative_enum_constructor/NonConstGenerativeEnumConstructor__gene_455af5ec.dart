// %before-language-feature: primary-constructors
enum E {
  v.named();
  E.named();
//^^^^^^^
// [diag.nonConstGenerativeEnumConstructor] Generative enum constructors must be 'const'.
}
