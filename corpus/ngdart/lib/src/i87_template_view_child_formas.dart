import 'package:ngdart/angular.dart';

/// Sonda: `@ViewChild` de `<template #t>` dentro de elemento e de um lido
/// pelo `*ngTemplateOutlet`.
@Component(
  selector: 'i87-template-view-child-formas',
  templateUrl: 'i87_template_view_child_formas.html',
  directives: [coreDirectives],
)
class I87TemplateViewChildFormas {
  @ViewChild('u')
  TemplateRef? usado;
  @ViewChild('t')
  TemplateRef? dentro;
}
