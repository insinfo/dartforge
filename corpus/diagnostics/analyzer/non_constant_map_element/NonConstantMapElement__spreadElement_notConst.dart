void main() {
  var notConst = {};
  const {1: null, ...notConst};
//                   ^^^^^^^^
// [diag.nonConstantMapElement] The elements in a const map literal must be constant.
}
