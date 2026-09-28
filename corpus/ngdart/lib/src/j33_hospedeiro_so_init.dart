import 'package:ngdart/angular.dart';

/// `@HostBinding` com um gancho só (`OnInit`) e `@Input` onPush.
@Component(
  selector: 'j33-hospedeiro-so-init',
  template: '<span>{{rotulo}}</span>',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J33HospedeiroSoInit implements OnInit {
  @Input()
  String rotulo = '';

  @HostBinding('class.pronto')
  bool pronto = false;

  @override
  void ngOnInit() {
    pronto = true;
  }
}
