// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j110_onpush_herda_input.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j110_onpush_herda_input.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngdart/src/runtime/text_binding.dart' as import11;
import 'package:ngdart/src/runtime/interpolate.dart' as import12;
import 'package:ngdart/src/devtools.dart' as import13;

final List<Object> styles$J110Botao = const [];

class ViewJ110Botao0 extends import0.ComponentView<import1.J110Botao> {
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ110Botao0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j110-botao'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j110_onpush_herda_input.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.classeDesligado;
    if (import8.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(this.rootElement, 'desligado', currVal_0);
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J110Botao, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J110BotaoNgFactory = ComponentFactory<import1.J110Botao>('j110-botao', viewFactory_J110BotaoHost0);
ComponentFactory<import1.J110Botao> get J110BotaoNgFactory {
  return _J110BotaoNgFactory;
}

ComponentFactory<import1.J110Botao> createJ110BotaoFactory() {
  return ComponentFactory('j110-botao', viewFactory_J110BotaoHost0);
}

final List<Object> styles$J110BotaoHost = const [];

class _ViewJ110BotaoHost0 extends import10.HostView<import1.J110Botao> {
  @override
  void build() {
    this.componentView = ViewJ110Botao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J110Botao();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool changed = false;
    bool firstCheck = this.firstCheck;
    if (changed) {
      this.componentView.markAsCheckOnce();
    }
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import10.HostView<import1.J110Botao> viewFactory_J110BotaoHost0() {
  return _ViewJ110BotaoHost0();
}

final List<Object> styles$J110Simples = const [];

class ViewJ110Simples0 extends import0.ComponentView<import1.J110Simples> {
  final import11.TextBinding _textBinding_1 = import11.TextBinding();
  static import2.ComponentStyles? _componentStyles;
  ViewJ110Simples0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j110-simples'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j110_onpush_herda_input.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import12.interpolateString0(_ctx.texto)) /* REF:asset:corpus_ngdart/lib/src/j110_onpush_herda_input.dart:647:658 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J110Simples, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J110SimplesNgFactory = ComponentFactory<import1.J110Simples>('j110-simples', viewFactory_J110SimplesHost0);
ComponentFactory<import1.J110Simples> get J110SimplesNgFactory {
  return _J110SimplesNgFactory;
}

ComponentFactory<import1.J110Simples> createJ110SimplesFactory() {
  return ComponentFactory('j110-simples', viewFactory_J110SimplesHost0);
}

final List<Object> styles$J110SimplesHost = const [];

class _ViewJ110SimplesHost0 extends import10.HostView<import1.J110Simples> {
  @override
  void build() {
    this.componentView = ViewJ110Simples0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J110Simples();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool changed = false;
    if (changed) {
      this.componentView.markAsCheckOnce();
    }
    this.componentView.detectChanges();
  }
}

import10.HostView<import1.J110Simples> viewFactory_J110SimplesHost0() {
  return _ViewJ110SimplesHost0();
}

final List<Object> styles$J110Usa = const [];

class ViewJ110Usa0 extends import0.ComponentView<import1.J110Usa> {
  late final ViewJ110Botao0 _compView_0;
  late final import1.J110Botao _J110Botao_0_5;
  late final ViewJ110Simples0 _compView_1;
  late final import1.J110Simples _J110Simples_1_5;
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ110Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j110-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j110_onpush_herda_input.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ110Botao0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J110Botao_0_5 = import1.J110Botao();
    this._compView_0.create(this._J110Botao_0_5);
    this._compView_1 = ViewJ110Simples0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    import7.setAttribute(_el_1, 'texto', 'a');
    this._J110Simples_1_5 = import1.J110Simples();
    this._compView_1.create(this._J110Simples_1_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool changed = false;
    bool firstCheck = this.firstCheck;
    changed = false;
    final currVal_0 = _ctx.d;
    if (import8.checkBinding(this._expr_0, currVal_0, 'd', 'asset:corpus_ngdart/lib/src/j110_onpush_herda_input.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J110Botao_0_5, 'desligado', currVal_0);
      }
      this._J110Botao_0_5.desligado = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j110_onpush_herda_input.dart:821:836 */;
      changed = true;
      this._expr_0 = currVal_0;
    }
    if (changed) {
      this._compView_0.markAsCheckOnce();
    }
    changed = false;
    if (firstCheck) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J110Simples_1_5, 'texto', 'a');
      }
      this._J110Simples_1_5.texto = 'a' /* REF:asset:corpus_ngdart/lib/src/j110_onpush_herda_input.dart:864:873 */;
      changed = true;
    }
    if (changed) {
      this._compView_1.markAsCheckOnce();
    }
    this._compView_0.detectHostChanges(firstCheck);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J110Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J110UsaNgFactory = ComponentFactory<import1.J110Usa>('j110-usa', viewFactory_J110UsaHost0);
ComponentFactory<import1.J110Usa> get J110UsaNgFactory {
  return _J110UsaNgFactory;
}

ComponentFactory<import1.J110Usa> createJ110UsaFactory() {
  return ComponentFactory('j110-usa', viewFactory_J110UsaHost0);
}

final List<Object> styles$J110UsaHost = const [];

class _ViewJ110UsaHost0 extends import10.HostView<import1.J110Usa> {
  @override
  void build() {
    this.componentView = ViewJ110Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J110Usa();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J110Usa> viewFactory_J110UsaHost0() {
  return _ViewJ110UsaHost0();
}
