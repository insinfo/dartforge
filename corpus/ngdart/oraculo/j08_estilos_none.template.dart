// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j08_estilos_none.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j08_estilos_none.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$J08EstilosNone = ['.a { color: red; }'];

class ViewJ08EstilosNone0 extends import0.ComponentView<import1.J08EstilosNone> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ08EstilosNone0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j08-estilos-none'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j08_estilos_none.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'a');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J08EstilosNone, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J08EstilosNoneNgFactory = ComponentFactory<import1.J08EstilosNone>('j08-estilos-none', viewFactory_J08EstilosNoneHost0);
ComponentFactory<import1.J08EstilosNone> get J08EstilosNoneNgFactory {
  return _J08EstilosNoneNgFactory;
}

ComponentFactory<import1.J08EstilosNone> createJ08EstilosNoneFactory() {
  return ComponentFactory('j08-estilos-none', viewFactory_J08EstilosNoneHost0);
}

final List<Object> styles$J08EstilosNoneHost = const [];

class _ViewJ08EstilosNoneHost0 extends import9.HostView<import1.J08EstilosNone> {
  @override
  void build() {
    this.componentView = ViewJ08EstilosNone0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J08EstilosNone();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J08EstilosNone> viewFactory_J08EstilosNoneHost0() {
  return _ViewJ08EstilosNoneHost0();
}
