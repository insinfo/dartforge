import 'package:ngdart/angular.dart';

/// Folha com `@import` (o `extractStyleUrls` do `StylesheetCompiler`): o
/// relativo vira o módulo da folha importada, antes do texto; o de URL com
/// esquema fica no texto.
@Component(
  selector: 'j36-estilo-importa',
  template: '<i class="a">x</i>',
  styleUrls: ['j36_estilo_importa.css'],
)
class J36EstiloImporta {}
