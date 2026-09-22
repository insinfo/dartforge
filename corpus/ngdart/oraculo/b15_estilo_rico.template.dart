// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b15_estilo_rico.dart';
import 'package:corpus_ngdart/src/b15_estilo_rico.css.shim.dart' as import0;
import 'package:ngdart/src/core/linker/views/component_view.dart' as import1;
import 'b15_estilo_rico.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$B15EstiloRico = [import0.styles];

class ViewB15EstiloRico0 extends import1.ComponentView<import2.B15EstiloRico> {
  static import3.ComponentStyles? _componentStyles;
  ViewB15EstiloRico0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('b15-estilo-rico'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/b15_estilo_rico.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'a');
    this.addShimC(_el_0);
    final _el_1 = import8.appendSpan(doc, _el_0);
    this.updateChildClass(_el_1, 'b');
    this.addShimC(_el_1);
    final _text_2 = import8.appendText(_el_1, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.scoped(styles$B15EstiloRico, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B15EstiloRicoNgFactory = ComponentFactory<import2.B15EstiloRico>('b15-estilo-rico', viewFactory_B15EstiloRicoHost0);
ComponentFactory<import2.B15EstiloRico> get B15EstiloRicoNgFactory {
  return _B15EstiloRicoNgFactory;
}

ComponentFactory<import2.B15EstiloRico> createB15EstiloRicoFactory() {
  return ComponentFactory('b15-estilo-rico', viewFactory_B15EstiloRicoHost0);
}

final List<Object> styles$B15EstiloRicoHost = const [];

class _ViewB15EstiloRicoHost0 extends import10.HostView<import2.B15EstiloRico> {
  @override
  void build() {
    this.componentView = ViewB15EstiloRico0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import2.B15EstiloRico();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import2.B15EstiloRico> viewFactory_B15EstiloRicoHost0() {
  return _ViewB15EstiloRicoHost0();
}
