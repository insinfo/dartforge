// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j41_injecao_no_conteudo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j41_injecao_no_conteudo.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/di/errors.dart' as import12;
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;

final List<Object> styles$J41Aba = const [];

class ViewJ41Aba0 extends import0.ComponentView<import1.J41Aba> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ41Aba0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j41-aba'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j41_injecao_no_conteudo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.project(_el_0, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J41Aba, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J41AbaNgFactory = ComponentFactory<import1.J41Aba>('j41-aba', viewFactory_J41AbaHost0);
ComponentFactory<import1.J41Aba> get J41AbaNgFactory {
  return _J41AbaNgFactory;
}

ComponentFactory<import1.J41Aba> createJ41AbaFactory() {
  return ComponentFactory('j41-aba', viewFactory_J41AbaHost0);
}

final List<Object> styles$J41AbaHost = const [];

class _ViewJ41AbaHost0 extends import9.HostView<import1.J41Aba> {
  @override
  void build() {
    this.componentView = ViewJ41Aba0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J41Aba();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J41Aba) && (0 == nodeIndex))) {
      return this.component;
    }
    return notFoundResult;
  }
}

import9.HostView<import1.J41Aba> viewFactory_J41AbaHost0() {
  return _ViewJ41AbaHost0();
}

final List<Object> styles$J41Local = const [];

class ViewJ41Local0 extends import0.ComponentView<import1.J41Local> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ41Local0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j41-local'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j41_injecao_no_conteudo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    this.project(_el_0, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J41Local, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J41LocalNgFactory = ComponentFactory<import1.J41Local>('j41-local', viewFactory_J41LocalHost0);
ComponentFactory<import1.J41Local> get J41LocalNgFactory {
  return _J41LocalNgFactory;
}

ComponentFactory<import1.J41Local> createJ41LocalFactory() {
  return ComponentFactory('j41-local', viewFactory_J41LocalHost0);
}

final List<Object> styles$J41LocalHost = const [];

class _ViewJ41LocalHost0 extends import9.HostView<import1.J41Local> {
  @override
  void build() {
    this.componentView = ViewJ41Local0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J41Local();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J41Local> viewFactory_J41LocalHost0() {
  return _ViewJ41LocalHost0();
}

final List<Object> styles$J41InjecaoNoConteudo = const [];

class ViewJ41InjecaoNoConteudo0 extends import0.ComponentView<import1.J41InjecaoNoConteudo> {
  late final ViewJ41Aba0 _compView_0;
  late final import1.J41Aba _J41Aba_0_5;
  late final import1.J41Dir _J41Dir_1_5;
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  late final ViewJ41Local0 _compView_4;
  late final import1.J41Local _J41Local_4_5;
  late final import1.J41Opc _J41Opc_5_5;
  late final import1.J41Pai _J41Pai_7_5;
  late final import1.J41Filha _J41Filha_8_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ41InjecaoNoConteudo0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j41-injecao-no-conteudo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j41_injecao_no_conteudo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ41Aba0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J41Aba_0_5 = import1.J41Aba();
    final doc = import6.document;
    final _el_1 = import5.unsafeCast(doc.createElement('span'));
    import7.setAttribute(_el_1, 'j41-dir', '');
    this._J41Dir_1_5 = (import5.isDevMode
        ? import12.debugInjectorWrap(import1.J41Dir, () {
            return import1.J41Dir(this._J41Aba_0_5, (this.parentView!).injectorGet(import1.J41Servico, this.parentIndex));
          })
        : import1.J41Dir(this._J41Aba_0_5, (this.parentView!).injectorGet(import1.J41Servico, this.parentIndex)));
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_1, this._J41Dir_1_5);
    }
    final _text_2 = import7.appendText(_el_1, 'x');
    final _anchor_3 = import7.createAnchor();
    this._appEl_3 = ViewContainer(3, 0, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_J41InjecaoNoConteudo1);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
    this._compView_0.createAndProject(this._J41Aba_0_5, [
      <Object>[_el_1, this._appEl_3]
    ]);
    this._compView_4 = ViewJ41Local0(this, 4);
    final _el_4 = this._compView_4.rootElement;
    parentRenderNode.append(_el_4);
    this._J41Local_4_5 = import1.J41Local();
    final _el_5 = import5.unsafeCast(doc.createElement('i'));
    import7.setAttribute(_el_5, 'j41-opc', '');
    this._J41Opc_5_5 = import1.J41Opc(this._J41Local_4_5);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_5, this._J41Opc_5_5);
    }
    final _text_6 = import7.appendText(_el_5, 'y');
    this._compView_4.createAndProject(this._J41Local_4_5, [
      <Object>[_el_5]
    ]);
    final _el_7 = import7.appendDiv(doc, parentRenderNode);
    import7.setAttribute(_el_7, 'j41-pai', '');
    this._J41Pai_7_5 = import1.J41Pai();
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_7, this._J41Pai_7_5);
    }
    final _el_8 = import7.appendElement<import6.HtmlElement>(doc, _el_7, 'em');
    import7.setAttribute(_el_8, 'j41-filha', '');
    this._J41Filha_8_5 = import1.J41Filha(this._J41Pai_7_5);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_8, this._J41Filha_8_5);
    }
    final _text_9 = import7.appendText(_el_8, 'w');
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J41Aba) && (nodeIndex <= 3))) {
      return this._J41Aba_0_5;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_3_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j41_injecao_no_conteudo.html:34:49 */;
    this._appEl_3.detectChangesInNestedViews();
    this._compView_0.detectChanges();
    this._compView_4.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
    this._compView_0.destroyInternalState();
    this._compView_4.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J41InjecaoNoConteudo, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J41InjecaoNoConteudoNgFactory = ComponentFactory<import1.J41InjecaoNoConteudo>('j41-injecao-no-conteudo', viewFactory_J41InjecaoNoConteudoHost0);
ComponentFactory<import1.J41InjecaoNoConteudo> get J41InjecaoNoConteudoNgFactory {
  return _J41InjecaoNoConteudoNgFactory;
}

ComponentFactory<import1.J41InjecaoNoConteudo> createJ41InjecaoNoConteudoFactory() {
  return ComponentFactory('j41-injecao-no-conteudo', viewFactory_J41InjecaoNoConteudoHost0);
}

class _ViewJ41InjecaoNoConteudo1 extends import15.EmbeddedView<import1.J41InjecaoNoConteudo> {
  late final import1.J41Dir _J41Dir_0_5;
  _ViewJ41InjecaoNoConteudo1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('b'));
    import7.setAttribute(_el_0, 'j41-dir', '');
    this._J41Dir_0_5 = (import5.isDevMode
        ? import12.debugInjectorWrap(import1.J41Dir, () {
            return import1.J41Dir(import5.unsafeCast<ViewJ41InjecaoNoConteudo0>((this.parentView!))._J41Aba_0_5, ((this.parentView!).parentView!).injectorGet(import1.J41Servico, (this.parentView!).parentIndex));
          })
        : import1.J41Dir(import5.unsafeCast<ViewJ41InjecaoNoConteudo0>((this.parentView!))._J41Aba_0_5, ((this.parentView!).parentView!).injectorGet(import1.J41Servico, (this.parentView!).parentIndex)));
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_0, this._J41Dir_0_5);
    }
    final _text_1 = import7.appendText(_el_0, 'z');
    this.initRootNode(_el_0);
  }
}

import15.EmbeddedView<void> viewFactory_J41InjecaoNoConteudo1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ41InjecaoNoConteudo1(parentView, parentIndex);
}

final List<Object> styles$J41InjecaoNoConteudoHost = const [];

class _ViewJ41InjecaoNoConteudoHost0 extends import9.HostView<import1.J41InjecaoNoConteudo> {
  @override
  void build() {
    this.componentView = ViewJ41InjecaoNoConteudo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J41InjecaoNoConteudo();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J41InjecaoNoConteudo> viewFactory_J41InjecaoNoConteudoHost0() {
  return _ViewJ41InjecaoNoConteudoHost0();
}
