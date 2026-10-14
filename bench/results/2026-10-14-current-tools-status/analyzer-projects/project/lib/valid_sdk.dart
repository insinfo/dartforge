import 'dart:async';
import 'dart:io';
import 'dart:convert';
import 'dart:typed_data';
Future<void> main() async { final bytes = Uint8List.fromList([65]); print(jsonEncode({'path': Directory.current.path, 'bytes': bytes})); await Future<void>.delayed(Duration.zero); }
// Audit fixture: analyzer-projects
