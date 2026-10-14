// SDK exceptions must cross optimized JSON frames and reach the caller's
// catch/finally. In macOS arm64 LTO, a missing FDE used to abort the process.
import 'dart:convert';

void main() {
  var caught = 0;
  var finalized = 0;
  for (final source in ['[1,', '{"x":}', '"unfinished', '[01]', '']) {
    try {
      jsonDecode(source);
      print('unexpected success');
    } on FormatException catch (error) {
      caught++;
      print('${error.message} @${error.offset}');
    } finally {
      finalized++;
    }
  }
  try {
    utf8.decoder.fuse(const JsonDecoder()).convert([0x5b, 0x31, 0x2c]);
  } on FormatException catch (error) {
    caught++;
    print('utf8: ${error.message} @${error.offset}');
  } finally {
    finalized++;
  }
  try {
    jsonEncode(double.infinity);
  } on JsonUnsupportedObjectError {
    caught++;
    print('unsupported number');
  } finally {
    finalized++;
  }
  print('caught=$caught finalized=$finalized');
  print(jsonEncode(jsonDecode('{"alive":[true,1,"text"]}')));
}
