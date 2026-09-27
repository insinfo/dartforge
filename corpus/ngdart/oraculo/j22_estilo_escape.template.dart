// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j22_estilo_escape.dart';
import 'package:corpus_ngdart/src/j22_estilo_escape.css.shim.dart' as import0;
import 'package:ngdart/src/core/linker/views/component_view.dart' as import1;
import 'j22_estilo_escape.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$J22EstiloEscape = [import0.styles];

class ViewJ22EstiloEscape0 extends import1.ComponentView<import2.J22EstiloEscape> {
  static import3.ComponentStyles? _componentStyles;
  ViewJ22EstiloEscape0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j22-estilo-escape'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j22_estilo_escape.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'i');
    this.updateChildClass(_el_0, 'icone');
    this.addShimC(_el_0);
    final _el_1 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'b');
    this.updateChildClass(_el_1, 'aspas');
    this.addShimC(_el_1);
    final _text_2 = import8.appendText(_el_1, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.scoped(styles$J22EstiloEscape, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J22EstiloEscapeNgFactory = ComponentFactory<import2.J22EstiloEscape>('j22-estilo-escape', viewFactory_J22EstiloEscapeHost0);
ComponentFactory<import2.J22EstiloEscape> get J22EstiloEscapeNgFactory {
  return _J22EstiloEscapeNgFactory;
}

ComponentFactory<import2.J22EstiloEscape> createJ22EstiloEscapeFactory() {
  return ComponentFactory('j22-estilo-escape', viewFactory_J22EstiloEscapeHost0);
}

final List<Object> styles$J22EstiloEscapeHost = const [];

class _ViewJ22EstiloEscapeHost0 extends import10.HostView<import2.J22EstiloEscape> {
  @override
  void build() {
    this.componentView = ViewJ22EstiloEscape0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import2.J22EstiloEscape();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import2.J22EstiloEscape> viewFactory_J22EstiloEscapeHost0() {
  return _ViewJ22EstiloEscapeHost0();
}
