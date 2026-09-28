// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j109_ref_exportado_no_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j109_ref_exportado_no_filho.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/text_binding.dart' as import10;
import 'package:ngdart/src/runtime/interpolate.dart' as import11;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/devtools.dart' as import14;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/runtime/check_binding.dart' as import16;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;

final List<Object> styles$J109Botao = const [];

class ViewJ109Botao0 extends import0.ComponentView<import1.J109Botao> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ109Botao0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j109-botao'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j109_ref_exportado_no_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, 'b');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J109Botao, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J109BotaoNgFactory = ComponentFactory<import1.J109Botao>('j109-botao', viewFactory_J109BotaoHost0);
ComponentFactory<import1.J109Botao> get J109BotaoNgFactory {
  return _J109BotaoNgFactory;
}

ComponentFactory<import1.J109Botao> createJ109BotaoFactory() {
  return ComponentFactory('j109-botao', viewFactory_J109BotaoHost0);
}

final List<Object> styles$J109BotaoHost = const [];

class _ViewJ109BotaoHost0 extends import9.HostView<import1.J109Botao> {
  @override
  void build() {
    this.componentView = ViewJ109Botao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J109Botao();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J109Botao> viewFactory_J109BotaoHost0() {
  return _ViewJ109BotaoHost0();
}

final List<Object> styles$J109Alvo = const [];

class ViewJ109Alvo0 extends import0.ComponentView<import1.J109Alvo> {
  final import10.TextBinding _textBinding_1 = import10.TextBinding();
  static import2.ComponentStyles? _componentStyles;
  ViewJ109Alvo0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j109-alvo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j109_ref_exportado_no_filho.dart' : null);
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
    this._textBinding_1.updateText(import11.interpolateString0(_ctx.fonte?.nome)) /* REF:asset:corpus_ngdart/lib/src/j109_ref_exportado_no_filho.dart:686:703 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J109Alvo, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J109AlvoNgFactory = ComponentFactory<import1.J109Alvo>('j109-alvo', viewFactory_J109AlvoHost0);
ComponentFactory<import1.J109Alvo> get J109AlvoNgFactory {
  return _J109AlvoNgFactory;
}

ComponentFactory<import1.J109Alvo> createJ109AlvoFactory() {
  return ComponentFactory('j109-alvo', viewFactory_J109AlvoHost0);
}

final List<Object> styles$J109AlvoHost = const [];

class _ViewJ109AlvoHost0 extends import9.HostView<import1.J109Alvo> {
  @override
  void build() {
    this.componentView = ViewJ109Alvo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J109Alvo();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J109Alvo> viewFactory_J109AlvoHost0() {
  return _ViewJ109AlvoHost0();
}

final List<Object> styles$J109Usa = const [];

class ViewJ109Usa0 extends import0.ComponentView<import1.J109Usa> {
  late final ViewJ109Botao0 _compView_0;
  late final import1.J109Botao _J109Botao_0_5;
  late final import1.J109Fonte _J109Fonte_0_6;
  late final ViewJ109Alvo0 _compView_1;
  late final import1.J109Alvo _J109Alvo_1_5;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ109Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j109-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j109_ref_exportado_no_filho.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ109Botao0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    import7.setAttribute(_el_0, 'j109Fonte', '');
    this._J109Botao_0_5 = import1.J109Botao();
    this._J109Fonte_0_6 = import1.J109Fonte();
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_el_0, this._J109Fonte_0_6);
    }
    this._compView_0.create(this._J109Botao_0_5);
    this._compView_1 = ViewJ109Alvo0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._J109Alvo_1_5 = import1.J109Alvo();
    this._compView_1.create(this._J109Alvo_1_5);
    final _anchor_2 = import7.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J109Usa1);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    _ctx.fonte = this._J109Fonte_0_6;
    import3.View.queryChangeDetectorRefs[this._J109Botao_0_5] = this._compView_0;
    _ctx.botao = this._J109Botao_0_5;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_origem = this._J109Fonte_0_6;
    final currVal_0 = local_origem;
    if (import16.checkBinding(this._expr_0, currVal_0, 'origem', 'asset:corpus_ngdart/lib/src/j109_ref_exportado_no_filho.dart')) {
      if (import14.isDevToolsEnabled) {
        import14.Inspector.instance.recordInput(this._J109Alvo_1_5, 'fonte', currVal_0);
      }
      this._J109Alvo_1_5.fonte = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j109_ref_exportado_no_filho.dart:885:901 */;
      this._expr_0 = currVal_0;
    }
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_2_9.ngIf = _ctx.mostra /* REF:asset:corpus_ngdart/lib/src/j109_ref_exportado_no_filho.dart:920:934 */;
    this._appEl_2.detectChangesInNestedViews();
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J109Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J109UsaNgFactory = ComponentFactory<import1.J109Usa>('j109-usa', viewFactory_J109UsaHost0);
ComponentFactory<import1.J109Usa> get J109UsaNgFactory {
  return _J109UsaNgFactory;
}

ComponentFactory<import1.J109Usa> createJ109UsaFactory() {
  return ComponentFactory('j109-usa', viewFactory_J109UsaHost0);
}

class _ViewJ109Usa1 extends import17.EmbeddedView<import1.J109Usa> {
  late final ViewJ109Alvo0 _compView_1;
  late final import1.J109Alvo _J109Alvo_1_5;
  late final ViewJ109Botao0 _compView_2;
  late final import1.J109Botao _J109Botao_2_5;
  Object? _expr_0;
  _ViewJ109Usa1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('div'));
    this._compView_1 = ViewJ109Alvo0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._J109Alvo_1_5 = import1.J109Alvo();
    this._compView_1.create(this._J109Alvo_1_5);
    this._compView_2 = ViewJ109Botao0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    _el_0.append(_el_2);
    this._J109Botao_2_5 = import1.J109Botao();
    this._compView_2.create(this._J109Botao_2_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_origem = import5.unsafeCast<ViewJ109Usa0>((this.parentView!))._J109Fonte_0_6;
    final currVal_0 = local_origem;
    if (import16.checkBinding(this._expr_0, currVal_0, 'origem', 'asset:corpus_ngdart/lib/src/j109_ref_exportado_no_filho.dart')) {
      if (import14.isDevToolsEnabled) {
        import14.Inspector.instance.recordInput(this._J109Alvo_1_5, 'fonte', currVal_0);
      }
      this._J109Alvo_1_5.fonte = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j109_ref_exportado_no_filho.dart:949:965 */;
      this._expr_0 = currVal_0;
    }
    this._compView_1.detectChanges();
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
    this._compView_2.destroyInternalState();
  }
}

import17.EmbeddedView<void> viewFactory_J109Usa1(import18.RenderView parentView, int parentIndex) {
  return _ViewJ109Usa1(parentView, parentIndex);
}

final List<Object> styles$J109UsaHost = const [];

class _ViewJ109UsaHost0 extends import9.HostView<import1.J109Usa> {
  @override
  void build() {
    this.componentView = ViewJ109Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J109Usa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J109Usa> viewFactory_J109UsaHost0() {
  return _ViewJ109UsaHost0();
}
