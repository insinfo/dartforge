// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j108_skip_self_no_proprio_no.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j108_skip_self_no_proprio_no.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/di/errors.dart' as import10;

final List<Object> styles$J108Dica = const [];

class ViewJ108Dica0 extends import0.ComponentView<import1.J108Dica> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ108Dica0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j108-dica'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j108_skip_self_no_proprio_no.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J108Dica, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J108DicaNgFactory = ComponentFactory<import1.J108Dica>('j108-dica', viewFactory_J108DicaHost0);
ComponentFactory<import1.J108Dica> get J108DicaNgFactory {
  return _J108DicaNgFactory;
}

ComponentFactory<import1.J108Dica> createJ108DicaFactory() {
  return ComponentFactory('j108-dica', viewFactory_J108DicaHost0);
}

final List<Object> styles$J108DicaHost = const [];

class _ViewJ108DicaHost0 extends import9.HostView<import1.J108Dica> {
  late final dynamic _J108Controle_0_5;
  @override
  void build() {
    this.componentView = ViewJ108Dica0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._J108Controle_0_5 = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J108Controle, () {
            return import1.criarControle(this.injectorGetOptional(import1.J108Controle, this.parentIndex));
          })
        : import1.criarControle(this.injectorGetOptional(import1.J108Controle, this.parentIndex)));
    this.component = import1.J108Dica(this._J108Controle_0_5);
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J108Controle) && (0 == nodeIndex))) {
      return this._J108Controle_0_5;
    }
    return notFoundResult;
  }
}

import9.HostView<import1.J108Dica> viewFactory_J108DicaHost0() {
  return _ViewJ108DicaHost0();
}

final List<Object> styles$J108JanelaComponente = const [];

class ViewJ108JanelaComponente0 extends import0.ComponentView<import1.J108JanelaComponente> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ108JanelaComponente0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j108-janela'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j108_skip_self_no_proprio_no.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, 'y');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J108JanelaComponente, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J108JanelaComponenteNgFactory = ComponentFactory<import1.J108JanelaComponente>('j108-janela', viewFactory_J108JanelaComponenteHost0);
ComponentFactory<import1.J108JanelaComponente> get J108JanelaComponenteNgFactory {
  return _J108JanelaComponenteNgFactory;
}

ComponentFactory<import1.J108JanelaComponente> createJ108JanelaComponenteFactory() {
  return ComponentFactory('j108-janela', viewFactory_J108JanelaComponenteHost0);
}

final List<Object> styles$J108JanelaComponenteHost = const [];

class _ViewJ108JanelaComponenteHost0 extends import9.HostView<import1.J108JanelaComponente> {
  @override
  void build() {
    this.componentView = ViewJ108JanelaComponente0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J108JanelaComponente, () {
            return import1.J108JanelaComponente(this.injectorGetOptional(import1.J108Janela, this.parentIndex));
          })
        : import1.J108JanelaComponente(this.injectorGetOptional(import1.J108Janela, this.parentIndex)));
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J108Janela) && (0 == nodeIndex))) {
      return this.component;
    }
    return notFoundResult;
  }
}

import9.HostView<import1.J108JanelaComponente> viewFactory_J108JanelaComponenteHost0() {
  return _ViewJ108JanelaComponenteHost0();
}

final List<Object> styles$J108Usa = const [];

class ViewJ108Usa0 extends import0.ComponentView<import1.J108Usa> {
  late final ViewJ108Dica0 _compView_0;
  late final dynamic _J108Controle_0_5;
  late final import1.J108Dica _J108Dica_0_6;
  late final ViewJ108JanelaComponente0 _compView_1;
  late final import1.J108JanelaComponente _J108JanelaComponente_1_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ108Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j108-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j108_skip_self_no_proprio_no.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ108Dica0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J108Controle_0_5 = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J108Controle, () {
            return import1.criarControle((this.parentView!).injectorGetOptional(import1.J108Controle, this.parentIndex));
          })
        : import1.criarControle((this.parentView!).injectorGetOptional(import1.J108Controle, this.parentIndex)));
    this._J108Dica_0_6 = import1.J108Dica(this._J108Controle_0_5);
    this._compView_0.create(this._J108Dica_0_6);
    this._compView_1 = ViewJ108JanelaComponente0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._J108JanelaComponente_1_5 = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J108JanelaComponente, () {
            return import1.J108JanelaComponente((this.parentView!).injectorGetOptional(import1.J108Janela, this.parentIndex));
          })
        : import1.J108JanelaComponente((this.parentView!).injectorGetOptional(import1.J108Janela, this.parentIndex)));
    this._compView_1.create(this._J108JanelaComponente_1_5);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J108Controle) && (0 == nodeIndex))) {
      return this._J108Controle_0_5;
    }
    if ((identical(token, import1.J108Janela) && (1 == nodeIndex))) {
      return this._J108JanelaComponente_1_5;
    }
    return notFoundResult;
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

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J108Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J108UsaNgFactory = ComponentFactory<import1.J108Usa>('j108-usa', viewFactory_J108UsaHost0);
ComponentFactory<import1.J108Usa> get J108UsaNgFactory {
  return _J108UsaNgFactory;
}

ComponentFactory<import1.J108Usa> createJ108UsaFactory() {
  return ComponentFactory('j108-usa', viewFactory_J108UsaHost0);
}

final List<Object> styles$J108UsaHost = const [];

class _ViewJ108UsaHost0 extends import9.HostView<import1.J108Usa> {
  @override
  void build() {
    this.componentView = ViewJ108Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J108Usa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J108Usa> viewFactory_J108UsaHost0() {
  return _ViewJ108UsaHost0();
}
