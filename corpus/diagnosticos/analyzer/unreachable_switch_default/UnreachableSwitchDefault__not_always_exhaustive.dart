String f(List x) {
  switch (x) {
    case []:
      return 'empty';
    case [var y, ...]:
      return 'non-empty starting with $y';
    default:
      return 'impossible';
  }
}
