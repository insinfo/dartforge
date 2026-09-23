void main() {
  bool notConst = true;
  const {1: null, if (notConst) null: null};
//                    ^^^^^^^^
// [diag.nonConstantMapElement] The elements in a const map literal must be constant.
}
