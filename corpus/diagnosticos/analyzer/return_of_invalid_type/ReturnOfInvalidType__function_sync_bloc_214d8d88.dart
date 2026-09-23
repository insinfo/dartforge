f() {
  try {
    return 0;
  } on ArgumentError {
    return 'abc';
  }
}
