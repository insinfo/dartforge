void f((int,) r) {
  r = (1);
//    ^^^
// [diag.recordLiteralOnePositionalNoTrailingCommaByType] A record literal with exactly one positional field requires a trailing comma.
}
