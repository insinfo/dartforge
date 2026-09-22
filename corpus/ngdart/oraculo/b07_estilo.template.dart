// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b07_estilo.dart';
import 'package:corpus_ngdart/src/b07_estilo.css.shim.dart' as import0;
import 'package:ngdart/src/core/linker/views/component_view.dart' as import1;
import 'b07_estilo.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$B07Estilo = [import0.styles];

class ViewB07Estilo0 extends import1.ComponentView<import2.B07Estilo> {
  static import3.ComponentStyles? _componentStyles;
  ViewB07Estilo0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('b07-estilo'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/b07_estilo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'c');
    this.addShimC(_el_0);
    final _el_1 = import8.appendSpan(doc, _el_0);
    this.addShimC(_el_1);
    final _text_2 = import8.appendText(_el_1, 'oi');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.scoped(styles$B07Estilo, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B07EstiloNgFactory = ComponentFactory<import2.B07Estilo>('b07-estilo', viewFactory_B07EstiloHost0);
ComponentFactory<import2.B07Estilo> get B07EstiloNgFactory {
  return _B07EstiloNgFactory;
}

ComponentFactory<import2.B07Estilo> createB07EstiloFactory() {
  return ComponentFactory('b07-estilo', viewFactory_B07EstiloHost0);
}

final List<Object> styles$B07EstiloHost = const [];

class _ViewB07EstiloHost0 extends import10.HostView<import2.B07Estilo> {
  @override
  void build() {
    this.componentView = ViewB07Estilo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import2.B07Estilo();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import2.B07Estilo> viewFactory_B07EstiloHost0() {
  return _ViewB07EstiloHost0();
}
