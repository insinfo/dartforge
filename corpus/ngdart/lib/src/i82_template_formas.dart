import 'package:ngdart/angular.dart';

/// Sonda: `<template #t>` dentro de elemento e com interpolação, um
/// `<template>` sem referência, e `*ngTemplateOutlet` num `<ng-container>`.
@Component(
  selector: 'i82-template-formas',
  templateUrl: 'i82_template_formas.html',
  directives: [coreDirectives],
)
class I82TemplateFormas {
  String nome = 'a';
}
