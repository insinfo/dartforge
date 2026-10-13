const int? intConst = 0;
const String? stringConst = "";
const map = {intConst: null, 0: ?intConst, null: 1, stringConst: 1};
//           ^^^^^^^^
// [context 1] The first key with this value.
//                           ^
// [diag.equalKeysInConstMap][context 1] Two keys in a constant map literal can't be equal.
