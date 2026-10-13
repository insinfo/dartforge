import 'dart:async';
FutureOr<int?> f(Future f) async {}
//             ^
// [diag.bodyMightCompleteNormallyNullable] This function has a nullable return type of 'FutureOr<int?>', but ends without returning a value.
