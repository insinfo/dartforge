// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j58_filho_rotulo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j58_filho_rotulo.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$J58FilhoRotulo = const [];

class ViewJ58FilhoRotulo0 extends import0.ComponentView<import1.J58FilhoRotulo> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ58FilhoRotulo0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j58-filho-rotulo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j58_filho_rotulo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'b');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J58FilhoRotulo, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J58FilhoRotuloNgFactory = ComponentFactory<import1.J58FilhoRotulo>('j58-filho-rotulo', viewFactory_J58FilhoRotuloHost0);
ComponentFactory<import1.J58FilhoRotulo> get J58FilhoRotuloNgFactory {
  return _J58FilhoRotuloNgFactory;
}

ComponentFactory<import1.J58FilhoRotulo> createJ58FilhoRotuloFactory() {
  return ComponentFactory('j58-filho-rotulo', viewFactory_J58FilhoRotuloHost0);
}

final List<Object> styles$J58FilhoRotuloHost = const [];

class _ViewJ58FilhoRotuloHost0 extends import9.HostView<import1.J58FilhoRotulo> {
  @override
  void build() {
    this.componentView = ViewJ58FilhoRotulo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J58FilhoRotulo();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J58FilhoRotulo> viewFactory_J58FilhoRotuloHost0() {
  return _ViewJ58FilhoRotuloHost0();
}
