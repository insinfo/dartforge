const int? intConst = 0;
const String? stringConst = "";
const set = {null, intConst, "", ?stringConst};
//                           ^^
// [context 1] The first element with this value.
//                                ^^^^^^^^^^^
// [diag.equalElementsInConstSet][context 1] Two elements in a constant set literal can't be equal.
