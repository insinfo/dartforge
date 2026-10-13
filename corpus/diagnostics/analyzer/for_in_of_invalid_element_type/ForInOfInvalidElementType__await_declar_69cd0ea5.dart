f(Stream<String> stream) async {
  await for (int i in stream) {
//                    ^^^^^^
// [diag.forInOfInvalidElementType] The type 'Stream<String>' used in the 'for' loop must implement 'Stream' with a type argument that can be assigned to 'int'.
    i;
  }
}
