enum E {
  v;
  static int get foo;
//                  ^
// [diag.missingFunctionBody] A function body must be provided.
}
