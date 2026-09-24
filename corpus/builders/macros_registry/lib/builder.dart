import 'package:build/build.dart';
import 'package:dartforge_macros_builder/macros_builder.dart';
import 'package:json/json.dart';

Builder macroDeclarations(BuilderOptions _) => macroDeclarationsBuilder({
      'package:json/json.dart#JsonCodable': (_) => const JsonCodable(),
      'package:json/json.dart#JsonEncodable': (_) => const JsonEncodable(),
      'package:json/json.dart#JsonDecodable': (_) => const JsonDecodable(),
    });
