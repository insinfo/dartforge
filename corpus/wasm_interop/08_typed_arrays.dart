// Typed data: Dart lists copied into JavaScript typed arrays, JavaScript
// typed arrays, buffers and data views seen from Dart as live views (64-bit
// values as BigInt), and the dart:convert and String paths over them.
import 'dart:convert';
import 'dart:js_interop';
import 'dart:typed_data';

@JS('eval')
external JSAny? jsEval(String code);

void main() {
  final u8 = Uint8List.fromList([1, 2, 250, 255]);
  final j = u8.toJS;
  print((jsEval('(a) => a.constructor.name + ":" + Array.from(a).join(",")') as JSFunction)
      .callAsFunction(null, j));
  final back = j.toDart;
  print('${back.runtimeType} ${back.length} ${back[2]} $back');
  back[0] = 9;
  print((jsEval('(a) => a[0]') as JSFunction).callAsFunction(null, j));
  final f64 = (jsEval('new Float64Array([1.5, -2, 3e300])') as JSFloat64Array).toDart;
  print('$f64 ${f64.length} ${f64.sublist(1)}');
  final i32 = (jsEval('new Int32Array([1, -2, 2147483647])') as JSInt32Array).toDart;
  print('$i32 ${i32.reduce((a, b) => a + b)}');
  final u32 = (jsEval('new Uint32Array([4294967295, 7])') as JSUint32Array).toDart;
  print(u32);
  final buf = (jsEval('new Uint8Array([104, 105, 0xc3, 0xa9]).buffer') as JSArrayBuffer).toDart;
  print('${buf.lengthInBytes} ${utf8.decode(buf.asUint8List())}');
  final bd = ByteData(8)..setInt64(0, -5)..setFloat32(4, 1.5);
  final dv = bd.toJS;
  print((jsEval('(d) => d.byteLength + " " + d.getInt8(3)') as JSFunction).callAsFunction(null, dv));
  final dvBack = (jsEval('new DataView(new ArrayBuffer(16))') as JSDataView).toDart;
  dvBack.setUint64(0, 0x123456789abcdef0);
  dvBack.setInt64(8, -2);
  print('${dvBack.getUint64(0).toRadixString(16)} ${dvBack.getInt64(8)} ${dvBack.getUint32(0)}');
  print(String.fromCharCodes(back));
  final list = <JSAny?>[1.toJS, 'x'.toJS, null].toJS;
  print(list.toDart.length);
  print(u8.jsify().dartify().runtimeType);
}
