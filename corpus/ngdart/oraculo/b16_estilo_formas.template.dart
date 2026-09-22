// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b16_estilo_formas.dart';
import 'package:corpus_ngdart/src/b16_estilo_formas.css.shim.dart' as import0;
import 'package:ngdart/src/core/linker/views/component_view.dart' as import1;
import 'b16_estilo_formas.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$B16EstiloFormas = [import0.styles];

class ViewB16EstiloFormas0 extends import1.ComponentView<import2.B16EstiloFormas> {
  static import3.ComponentStyles? _componentStyles;
  ViewB16EstiloFormas0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('b16-estilo-formas'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/b16_estilo_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'a');
    this.addShimC(_el_0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.scoped(styles$B16EstiloFormas, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B16EstiloFormasNgFactory = ComponentFactory<import2.B16EstiloFormas>('b16-estilo-formas', viewFactory_B16EstiloFormasHost0);
ComponentFactory<import2.B16EstiloFormas> get B16EstiloFormasNgFactory {
  return _B16EstiloFormasNgFactory;
}

ComponentFactory<import2.B16EstiloFormas> createB16EstiloFormasFactory() {
  return ComponentFactory('b16-estilo-formas', viewFactory_B16EstiloFormasHost0);
}

final List<Object> styles$B16EstiloFormasHost = const [];

class _ViewB16EstiloFormasHost0 extends import10.HostView<import2.B16EstiloFormas> {
  @override
  void build() {
    this.componentView = ViewB16EstiloFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import2.B16EstiloFormas();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import2.B16EstiloFormas> viewFactory_B16EstiloFormasHost0() {
  return _ViewB16EstiloFormasHost0();
}
