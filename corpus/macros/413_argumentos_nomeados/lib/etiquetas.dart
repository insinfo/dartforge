import 'dart:convert';
import 'package:macros/macros.dart';

macro class Etiquetas implements ClassDeclarationsMacro {
  final String prefixo;
  const Etiquetas({required this.prefixo});

  @override
  Future<void> buildDeclarationsForClass(
      ClassDeclaration classe, MemberDeclarationBuilder builder) async {
    builder.declareInType(DeclarationCode.fromString(
        '  static const String primeiro = ${jsonEncode(prefixo)};'));
  }
}
