import 'dart:html' as html;

import 'package:ngdart/angular.dart';

import 'd08_servico.dart';

/// Filho que recebe serviço, serviço opcional, o elemento e o
/// `ChangeDetectorRef`.
@Component(
  selector: 'd08-filho-injetado',
  templateUrl: 'd08_filho_injetado.html',
)
class D08FilhoInjetado {
  final D08Servico servico;
  final D08Opcional? opcional;
  final html.Element elemento;
  final ChangeDetectorRef detector;

  D08FilhoInjetado(
    this.servico,
    @Optional() this.opcional,
    this.elemento,
    this.detector,
  );
}
