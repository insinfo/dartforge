// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j93_ouvinte_no_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j93_ouvinte_no_filho.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;

final List<Object> styles$J93Filho = const [];

class ViewJ93Filho0 extends import0.ComponentView<import1.J93Filho> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ93Filho0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j93-filho'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j93_ouvinte_no_filho.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    parentRenderNode.addEventListener('click', this.eventHandler0(_ctx.clicado));
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J93Filho, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J93FilhoNgFactory = ComponentFactory<import1.J93Filho>('j93-filho', viewFactory_J93FilhoHost0);
ComponentFactory<import1.J93Filho> get J93FilhoNgFactory {
  return _J93FilhoNgFactory;
}

ComponentFactory<import1.J93Filho> createJ93FilhoFactory() {
  return ComponentFactory('j93-filho', viewFactory_J93FilhoHost0);
}

final List<Object> styles$J93FilhoHost = const [];

class _ViewJ93FilhoHost0 extends import9.HostView<import1.J93Filho> {
  @override
  void build() {
    this.componentView = ViewJ93Filho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J93Filho();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J93Filho> viewFactory_J93FilhoHost0() {
  return _ViewJ93FilhoHost0();
}

final List<Object> styles$J93OuvinteNoFilho = const [];

class ViewJ93OuvinteNoFilho0 extends import0.ComponentView<import1.J93OuvinteNoFilho> {
  late final ViewJ93Filho0 _compView_0;
  late final import1.J93Filho _J93Filho_0_5;
  late final import1.J93Dica _J93Dica_0_6;
  late final ViewJ93Filho0 _compView_1;
  late final import1.J93Filho _J93Filho_1_5;
  late final import1.J93Dica _J93Dica_1_6;
  static import2.ComponentStyles? _componentStyles;
  ViewJ93OuvinteNoFilho0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j93-ouvinte-no-filho'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j93_ouvinte_no_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ93Filho0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    import7.setAttribute(_el_0, 'j93-dica', '');
    this._J93Filho_0_5 = import1.J93Filho();
    this._J93Dica_0_6 = import1.J93Dica();
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_0, this._J93Dica_0_6);
    }
    this._compView_0.create(this._J93Filho_0_5);
    this._compView_1 = ViewJ93Filho0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    import7.setAttribute(_el_1, 'j93-dica', '');
    this._J93Filho_1_5 = import1.J93Filho();
    this._J93Dica_1_6 = import1.J93Dica();
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_1, this._J93Dica_1_6);
    }
    this._compView_1.create(this._J93Filho_1_5);
    _el_0.addEventListener('mouseenter', this.eventHandler0(this._J93Dica_0_6.entrar));
    _el_0.addEventListener('click', this.eventHandler1(this._J93Dica_0_6.clicar));
    _el_1.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    _el_1.addEventListener('mouseenter', this.eventHandler0(this._J93Dica_1_6.entrar));
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.avisar();
    this._J93Dica_1_6.clicar($event);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J93OuvinteNoFilho, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J93OuvinteNoFilhoNgFactory = ComponentFactory<import1.J93OuvinteNoFilho>('j93-ouvinte-no-filho', viewFactory_J93OuvinteNoFilhoHost0);
ComponentFactory<import1.J93OuvinteNoFilho> get J93OuvinteNoFilhoNgFactory {
  return _J93OuvinteNoFilhoNgFactory;
}

ComponentFactory<import1.J93OuvinteNoFilho> createJ93OuvinteNoFilhoFactory() {
  return ComponentFactory('j93-ouvinte-no-filho', viewFactory_J93OuvinteNoFilhoHost0);
}

final List<Object> styles$J93OuvinteNoFilhoHost = const [];

class _ViewJ93OuvinteNoFilhoHost0 extends import9.HostView<import1.J93OuvinteNoFilho> {
  @override
  void build() {
    this.componentView = ViewJ93OuvinteNoFilho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J93OuvinteNoFilho();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J93OuvinteNoFilho> viewFactory_J93OuvinteNoFilhoHost0() {
  return _ViewJ93OuvinteNoFilhoHost0();
}
