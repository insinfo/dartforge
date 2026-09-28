import 'package:ngdart/angular.dart';

import 'j26_gatilho.dart';
import 'j27_caixa.dart';

/// `<template diretiva let-ctx>` projetado num filho, com e sem `let-`.
@Component(
  selector: 'j28-usa-caixa',
  templateUrl: 'j28_usa_caixa.html',
  directives: [J26Gatilho, J27Caixa],
)
class J28UsaCaixa {
  String titulo = 't';
}
