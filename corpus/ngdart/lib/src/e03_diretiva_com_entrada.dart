import 'package:ngdart/angular.dart';

/// `@Directive` com `@Input` e ciclo de vida.
@Directive(selector: '[e03-entrada]')
class E03DiretivaComEntrada implements OnInit {
  @Input()
  String valor = '';

  @override
  void ngOnInit() {}
}
