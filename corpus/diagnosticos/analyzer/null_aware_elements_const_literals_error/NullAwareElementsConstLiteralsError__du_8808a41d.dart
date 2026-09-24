const nullConst = null;
const int? intConst = 0;
const String? stringConst = "";
const map = {null: 1, nullConst: ?intConst, stringConst: 1};
//           ^^^^
// [context 1] The first key with this value.
//                    ^^^^^^^^^
// [diag.equalKeysInConstMap][context 1] Two keys in a constant map literal can't be equal.
