import 'package:ngdart/angular.dart';

/// Sonda: `<template>` com `[ngSwitchCase]`, `ngSwitchDefault` e `[ngIf]` com texto.
@Component(
  selector: 'i53-template-formas',
  templateUrl: 'i53_template_formas.html',
  directives: [coreDirectives],
)
class I53TemplateFormas {
  String modo = 'a';
  bool mostrar = true;
}
