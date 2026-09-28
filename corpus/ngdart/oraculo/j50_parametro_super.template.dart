// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j50_parametro_super.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j50_parametro_super.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/devtools.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$J50ParametroSuper = const [];

class ViewJ50ParametroSuper0 extends import0.ComponentView<import1.J50ParametroSuper> {
  late final import1.J50Pai _J50Pai_0_5;
  late final import1.J50Gatilho _J50Gatilho_1_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ50ParametroSuper0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j50-parametro-super'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j50_parametro_super.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    import7.setAttribute(_el_0, 'j50-pai', '');
    this._J50Pai_0_5 = import1.J50Pai();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_0, this._J50Pai_0_5);
    }
    final _el_1 = import7.appendElement<import6.ButtonElement>(doc, _el_0, 'button');
    import7.setAttribute(_el_1, 'j50-gatilho', '');
    this._J50Gatilho_1_5 = import1.J50Gatilho(this._J50Pai_0_5, _el_1);
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_1, this._J50Gatilho_1_5);
    }
    final _text_2 = import7.appendText(_el_1, 'x');
    _el_1.addEventListener('click', this.eventHandler0(this._J50Gatilho_1_5.clicar));
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (((!import9.debugThrowIfChanged) && firstCheck)) {
      this._J50Gatilho_1_5.ngOnInit();
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J50ParametroSuper, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J50ParametroSuperNgFactory = ComponentFactory<import1.J50ParametroSuper>('j50-parametro-super', viewFactory_J50ParametroSuperHost0);
ComponentFactory<import1.J50ParametroSuper> get J50ParametroSuperNgFactory {
  return _J50ParametroSuperNgFactory;
}

ComponentFactory<import1.J50ParametroSuper> createJ50ParametroSuperFactory() {
  return ComponentFactory('j50-parametro-super', viewFactory_J50ParametroSuperHost0);
}

final List<Object> styles$J50ParametroSuperHost = const [];

class _ViewJ50ParametroSuperHost0 extends import11.HostView<import1.J50ParametroSuper> {
  @override
  void build() {
    this.componentView = ViewJ50ParametroSuper0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J50ParametroSuper();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J50ParametroSuper> viewFactory_J50ParametroSuperHost0() {
  return _ViewJ50ParametroSuperHost0();
}
