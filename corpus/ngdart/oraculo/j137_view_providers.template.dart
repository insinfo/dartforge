// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j137_view_providers.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j137_view_providers.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/di/errors.dart' as import8;
import 'package:ngdart/src/devtools.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$J137Caixa = const [];

class ViewJ137Caixa0 extends import0.ComponentView<import1.J137Caixa> {
  late final import1.J137Leitor _J137Leitor_0_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ137Caixa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j137-caixa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j137_view_providers.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    import7.setAttribute(_el_0, 'j137Leitor', '');
    this._J137Leitor_0_5 = (import5.isDevMode
        ? import8.debugInjectorWrap(import1.J137Leitor, () {
            return import1.J137Leitor((this.parentView!).injectorGetOptional(import1.J137Modelo, this.parentIndex), null);
          })
        : import1.J137Leitor((this.parentView!).injectorGetOptional(import1.J137Modelo, this.parentIndex), null));
    if (import9.isDevToolsEnabled) {
      import9.Inspector.instance.registerDirective(_el_0, this._J137Leitor_0_5);
    }
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J137Caixa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J137CaixaNgFactory = ComponentFactory<import1.J137Caixa>('j137-caixa', viewFactory_J137CaixaHost0);
ComponentFactory<import1.J137Caixa> get J137CaixaNgFactory {
  return _J137CaixaNgFactory;
}

ComponentFactory<import1.J137Caixa> createJ137CaixaFactory() {
  return ComponentFactory('j137-caixa', viewFactory_J137CaixaHost0);
}

final List<Object> styles$J137CaixaHost = const [];

class _ViewJ137CaixaHost0 extends import11.HostView<import1.J137Caixa> {
  late import1.J137Modelo<dynamic> _J137Modelo_0_6 = import1.j137DoCaixa(this.component);
  @override
  void build() {
    this.componentView = ViewJ137Caixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J137Caixa();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J137Modelo) && (0 == nodeIndex))) {
      return this._J137Modelo_0_6;
    }
    return notFoundResult;
  }
}

import11.HostView<import1.J137Caixa> viewFactory_J137CaixaHost0() {
  return _ViewJ137CaixaHost0();
}
