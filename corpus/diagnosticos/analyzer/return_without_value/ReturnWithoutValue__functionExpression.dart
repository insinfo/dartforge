f() {
  return (int y) {
    if (y < 0) {
      return;
//    ^^^^^^
// [diag.returnWithoutValue] The return value is missing after 'return'.
    }
    return 0;
  };
}
