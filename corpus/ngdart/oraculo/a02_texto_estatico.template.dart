// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a02_texto_estatico.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a02_texto_estatico.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$A02TextoEstatico = const [];

class ViewA02TextoEstatico0 extends import0.ComponentView<import1.A02TextoEstatico> {
  static import2.ComponentStyles? _componentStyles;
  ViewA02TextoEstatico0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('a02-texto-estatico'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/a02_texto_estatico.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'c');
    final _el_1 = import7.appendSpan(doc, _el_0);
    final _text_2 = import7.appendText(_el_1, 'oi');
    final _el_3 = import7.appendElement<import6.HtmlElement>(doc, _el_0, 'img');
    import7.setAttribute(_el_3, 'height', '10');
    import7.setAttribute(_el_3, 'src', 'a.svg');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$A02TextoEstatico, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A02TextoEstaticoNgFactory = ComponentFactory<import1.A02TextoEstatico>('a02-texto-estatico', viewFactory_A02TextoEstaticoHost0);
ComponentFactory<import1.A02TextoEstatico> get A02TextoEstaticoNgFactory {
  return _A02TextoEstaticoNgFactory;
}

ComponentFactory<import1.A02TextoEstatico> createA02TextoEstaticoFactory() {
  return ComponentFactory('a02-texto-estatico', viewFactory_A02TextoEstaticoHost0);
}

final List<Object> styles$A02TextoEstaticoHost = const [];

class _ViewA02TextoEstaticoHost0 extends import9.HostView<import1.A02TextoEstatico> {
  @override
  void build() {
    this.componentView = ViewA02TextoEstatico0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A02TextoEstatico();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.A02TextoEstatico> viewFactory_A02TextoEstaticoHost0() {
  return _ViewA02TextoEstaticoHost0();
}
