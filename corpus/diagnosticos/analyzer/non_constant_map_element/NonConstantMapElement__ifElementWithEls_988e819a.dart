void main() {
  const isTrue = true;
  const {1: null, if (isTrue) null: null else null: null};
//                                            ^^^^^^^^^^
// [diag.deadCode] Dead code.
}
