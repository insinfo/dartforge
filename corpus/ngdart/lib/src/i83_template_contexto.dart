import 'package:ngdart/angular.dart';

/// Sonda: `*ngTemplateOutlet` com `context:` dentro de `*ngIf`, lendo o
/// `<template #t>` da visão de cima.
@Component(
  selector: 'i83-template-contexto',
  templateUrl: 'i83_template_contexto.html',
  directives: [coreDirectives],
)
class I83TemplateContexto {
  bool ver = true;
  Map<String, dynamic> ctx = {'x': 1};
}
