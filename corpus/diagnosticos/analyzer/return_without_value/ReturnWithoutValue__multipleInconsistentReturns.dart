int f(int x) {
  if (x < 0) {
    return 1;
  }
  return;
//^^^^^^
// [diag.returnWithoutValue] The return value is missing after 'return'.
}
