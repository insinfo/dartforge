import 'package:ngdart/angular.dart';

/// Sonda de recusa: `@ViewChild` de um `<template #t>` (o valor é o
/// `TemplateRef`).
@Component(
  selector: 'i85-template-view-child',
  template: '<template #t><p>x</p></template>',
)
class I85TemplateViewChild {
  @ViewChild('t')
  TemplateRef? molde;
}
