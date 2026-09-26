import 'package:ngdart/angular.dart';

/// Sonda: `*ngFor` com `trackBy`.
@Component(
  selector: 'i02-ng-for-track-by',
  templateUrl: 'i02_ng_for_track_by.html',
  directives: [coreDirectives],
)
class I02NgForTrackBy {
  List<String> itens = ['a', 'b'];
  Object? rastrear(int i, dynamic item) => item;
}
