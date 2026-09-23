// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b22_estilo_csslib.dart';
import 'package:corpus_ngdart/src/b22_estilo_csslib.css.shim.dart' as import0;
import 'package:ngdart/src/core/linker/views/component_view.dart' as import1;
import 'b22_estilo_csslib.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$B22EstiloCsslib = [import0.styles];

class ViewB22EstiloCsslib0 extends import1.ComponentView<import2.B22EstiloCsslib> {
  static import3.ComponentStyles? _componentStyles;
  ViewB22EstiloCsslib0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('b22-estilo-csslib'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/b22_estilo_csslib.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'a');
    this.addShimC(_el_0);
    final _text_1 = import8.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.scoped(styles$B22EstiloCsslib, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B22EstiloCsslibNgFactory = ComponentFactory<import2.B22EstiloCsslib>('b22-estilo-csslib', viewFactory_B22EstiloCsslibHost0);
ComponentFactory<import2.B22EstiloCsslib> get B22EstiloCsslibNgFactory {
  return _B22EstiloCsslibNgFactory;
}

ComponentFactory<import2.B22EstiloCsslib> createB22EstiloCsslibFactory() {
  return ComponentFactory('b22-estilo-csslib', viewFactory_B22EstiloCsslibHost0);
}

final List<Object> styles$B22EstiloCsslibHost = const [];

class _ViewB22EstiloCsslibHost0 extends import10.HostView<import2.B22EstiloCsslib> {
  @override
  void build() {
    this.componentView = ViewB22EstiloCsslib0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import2.B22EstiloCsslib();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import2.B22EstiloCsslib> viewFactory_B22EstiloCsslibHost0() {
  return _ViewB22EstiloCsslibHost0();
}
