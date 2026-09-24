import 'dart:convert';
import 'package:macros/macros.dart';

macro class Rotulo implements ClassDeclarationsMacro {
  final String texto;
  const Rotulo(this.texto);

  @override
  Future<void> buildDeclarationsForClass(
      ClassDeclaration classe, MemberDeclarationBuilder builder) async {
    builder.declareInType(DeclarationCode.fromString(
        '  static const String rotulo = ${jsonEncode(texto)};'));
  }
}
