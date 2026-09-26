import 'package:ngdart/angular.dart';

/// Sonda: `<template [ngIf]>` escrito.
@Component(
  selector: 'i10-template-explicito',
  templateUrl: 'i10_template_explicito.html',
  directives: [coreDirectives],
)
class I10TemplateExplicito {
  bool mostrar = true;
}
