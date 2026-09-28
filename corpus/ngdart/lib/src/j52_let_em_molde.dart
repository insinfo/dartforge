import 'package:ngdart/angular.dart';

import 'j52_filho.dart';

/// `<template #t let-ctx>` sem diretiva, com o local lido no molde, num
/// `*ngIf` aninhado e num evento (o `wizardStepHeaderTemplate` do
/// limitless_ui).
@Component(
  selector: 'j52-let-em-molde',
  templateUrl: 'j52_let_em_molde.html',
  directives: [J52Filho, NgIf],
)
class J52LetEmMolde {
  String rotulo(Object? c) => '$c';
}
