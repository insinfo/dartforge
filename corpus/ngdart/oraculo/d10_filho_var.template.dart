// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd10_filho_var.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd10_filho_var.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$D10FilhoVar = const [];

class ViewD10FilhoVar0 extends import0.ComponentView<import1.D10FilhoVar> {
  static import2.ComponentStyles? _componentStyles;
  ViewD10FilhoVar0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('d10-filho-var'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/d10_filho_var.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'b');
    final _text_1 = import7.appendText(_el_0, 'f');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$D10FilhoVar, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D10FilhoVarNgFactory = ComponentFactory<import1.D10FilhoVar>('d10-filho-var', viewFactory_D10FilhoVarHost0);
ComponentFactory<import1.D10FilhoVar> get D10FilhoVarNgFactory {
  return _D10FilhoVarNgFactory;
}

ComponentFactory<import1.D10FilhoVar> createD10FilhoVarFactory() {
  return ComponentFactory('d10-filho-var', viewFactory_D10FilhoVarHost0);
}

final List<Object> styles$D10FilhoVarHost = const [];

class _ViewD10FilhoVarHost0 extends import9.HostView<import1.D10FilhoVar> {
  @override
  void build() {
    this.componentView = ViewD10FilhoVar0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D10FilhoVar();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.D10FilhoVar> viewFactory_D10FilhoVarHost0() {
  return _ViewD10FilhoVarHost0();
}
