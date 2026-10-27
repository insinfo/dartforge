// Record fields named constructor or prototype read statically and
// dynamically. A dynamic value passed where Never or Null is expected is
// cast.
void takesNever(Never n) {}

void apply(void Function(Never) fn, dynamic arg) {
  fn(arg);
}

void main() {
  var r = (constructor: 1, prototype: 2, shape: 3);
  dynamic d = r;
  print([r.constructor, r.prototype, r.shape]);
  print([d.constructor, d.prototype, d.shape]);
  try {
    apply(takesNever, 3);
    print('accepted');
  } on TypeError {
    print('TypeError');
  }
}
