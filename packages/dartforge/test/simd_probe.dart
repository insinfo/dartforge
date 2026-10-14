/// Executes the exported SIMD declaration on the VM and native AOT.
import 'dart:typed_data';

import 'package:dartforge/dartforge.dart';
import 'package:dartforge/simd.dart';

String lanes(Int32x4 value) => '${value.x},${value.y},${value.z},${value.w}';

void main() {
  final a = Int32x4(0x7fffffff, -0x80000000, 3, -7);
  final b = Int32x4(2, -1, -5, 4);
  print('multiply=${lanes(a * b)}');
  print('negate=${lanes(-a)}');
  print('abs=${lanes(a.abs())}');
  print('invert=${lanes(~a)}');
  print('andNot=${lanes(a.andNot(b))}');
  for (final shift in [-1, 0, 1, 31, 32, 63]) {
    print('left:$shift=${lanes(a << shift)}');
    print('right:$shift=${lanes(a >> shift)}');
  }
  print('equal=${lanes(a.equal(b))}');
  print('notEqual=${lanes(a.notEqual(b))}');
  print('less=${lanes(a.lessThan(b))}');
  print('lessEqual=${lanes(a.lessThanOrEqual(b))}');
  print('greater=${lanes(a.greaterThan(b))}');
  print('greaterEqual=${lanes(a.greaterThanOrEqual(b))}');
  print('min=${lanes(a.min(b))}');
  print('max=${lanes(a.max(b))}');
  final masks = [
    Int32x4(0, 0, 0, 0),
    Int32x4(-1, 0, -1, 0),
    Int32x4(-1, -1, -1, -1)
  ];
  for (final mask in masks) {
    print('truth:${lanes(mask)}=${mask.anyTrue},${mask.allTrue}');
  }
  if (lanes(a * b) != '-2,-2147483648,-15,-28') {
    throw StateError('SIMD multiplication must wrap each lane');
  }
  print('SIMD package verified');
}
