f(Stream<String> stream) async {
  int i;
  await for (i in stream) {
//                ^^^^^^
// [diag.forInOfInvalidElementType] The type 'Stream<String>' used in the 'for' loop must implement 'Stream' with a type argument that can be assigned to 'int'.
    i;
  }
}
