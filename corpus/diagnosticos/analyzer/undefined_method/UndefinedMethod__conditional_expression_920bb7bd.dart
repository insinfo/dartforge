T castObject<T>(Object value) => value as T;

main() {
  (castObject(true)..whatever()) ? 1 : 2;
//                   ^^^^^^^^
// [diag.undefinedMethod] The method 'whatever' isn't defined for the type 'bool'.
}
