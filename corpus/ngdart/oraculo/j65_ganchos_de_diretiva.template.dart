// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j65_ganchos_de_diretiva.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j65_ganchos_de_diretiva.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/runtime/dom_helpers.dart' as import12;
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import17;

final List<Object> styles$J65Filho = const [];

class ViewJ65Filho0 extends import0.ComponentView<import1.J65Filho> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ65Filho0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j65-filho'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j65_ganchos_de_diretiva.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J65Filho, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J65FilhoNgFactory = ComponentFactory<import1.J65Filho>('j65-filho', viewFactory_J65FilhoHost0);
ComponentFactory<import1.J65Filho> get J65FilhoNgFactory {
  return _J65FilhoNgFactory;
}

ComponentFactory<import1.J65Filho> createJ65FilhoFactory() {
  return ComponentFactory('j65-filho', viewFactory_J65FilhoHost0);
}

final List<Object> styles$J65FilhoHost = const [];

class _ViewJ65FilhoHost0 extends import8.HostView<import1.J65Filho> {
  @override
  void build() {
    this.componentView = ViewJ65Filho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J65Filho();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if ((!import9.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterContentInit();
      }
    }
    this.componentView.detectChanges();
  }

  @override
  void destroyInternal() {
    this.component.ngOnDestroy();
  }
}

import8.HostView<import1.J65Filho> viewFactory_J65FilhoHost0() {
  return _ViewJ65FilhoHost0();
}

final List<Object> styles$J65GanchosDeDiretiva = const [];

class ViewJ65GanchosDeDiretiva0 extends import0.ComponentView<import1.J65GanchosDeDiretiva> {
  late final import1.J65Todos _J65Todos_0_5;
  late final import1.J65Conteudo _J65Conteudo_1_5;
  late final ViewJ65Filho0 _compView_2;
  late final import1.J65Filho _J65Filho_2_5;
  late final import1.J65Conteudo _J65Conteudo_2_6;
  late final J65HospedeiroNgCd _J65Hospedeiro_3_5;
  late final ViewContainer _appEl_4;
  late final NgIf _NgIf_4_9;
  late final import1.J65Conteudo _J65Conteudo_5_5;
  late final J65HospedeiroNgCd _J65Hospedeiro_5_6;
  late final import6.HtmlElement _el_3;
  late final import6.HtmlElement _el_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ65GanchosDeDiretiva0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j65-ganchos-de-diretiva'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j65_ganchos_de_diretiva.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import12.appendDiv(doc, parentRenderNode);
    import12.setAttribute(_el_0, 'j65-todos', '');
    this._J65Todos_0_5 = import1.J65Todos();
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_0, this._J65Todos_0_5);
    }
    final _el_1 = import12.appendSpan(doc, _el_0);
    import12.setAttribute(_el_1, 'j65-conteudo', '');
    this._J65Conteudo_1_5 = import1.J65Conteudo();
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_1, this._J65Conteudo_1_5);
    }
    this._compView_2 = ViewJ65Filho0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    _el_0.append(_el_2);
    import12.setAttribute(_el_2, 'j65-conteudo', '');
    this._J65Filho_2_5 = import1.J65Filho();
    this._J65Conteudo_2_6 = import1.J65Conteudo();
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_2, this._J65Conteudo_2_6);
    }
    this._el_3 = import5.unsafeCast(doc.createElement('b'));
    import12.setAttribute(this._el_3, 'j65-hospedeiro', '');
    this._J65Hospedeiro_3_5 = J65HospedeiroNgCd(import1.J65Hospedeiro());
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(this._el_3, this._J65Hospedeiro_3_5.instance);
    }
    this._compView_2.createAndProject(this._J65Filho_2_5, [
      <Object>[this._el_3]
    ]);
    final _anchor_4 = import12.appendAnchor(parentRenderNode);
    this._appEl_4 = ViewContainer(4, null, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_J65GanchosDeDiretiva1);
    this._NgIf_4_9 = NgIf(this._appEl_4, _TemplateRef_4_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_4, this._NgIf_4_9);
    }
    this._el_5 = import12.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'em');
    import12.setAttribute(this._el_5, 'j65-conteudo', '');
    import12.setAttribute(this._el_5, 'j65-hospedeiro', '');
    this._J65Conteudo_5_5 = import1.J65Conteudo();
    this._J65Hospedeiro_5_6 = J65HospedeiroNgCd(import1.J65Hospedeiro());
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(this._el_5, this._J65Conteudo_5_5);
      import13.Inspector.instance.registerDirective(this._el_5, this._J65Hospedeiro_5_6.instance);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_4_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_4_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j65_ganchos_de_diretiva.html:124:139 */;
    this._appEl_4.detectChangesInNestedViews();
    if ((!import9.debugThrowIfChanged)) {
      if (firstCheck) {
        this._J65Conteudo_1_5.ngAfterContentInit();
        this._J65Filho_2_5.ngAfterContentInit();
        this._J65Conteudo_2_6.ngAfterContentInit();
        this._J65Todos_0_5.ngAfterContentInit();
      }
      this._J65Todos_0_5.ngAfterContentChecked();
      if (firstCheck) {
        this._J65Conteudo_5_5.ngAfterContentInit();
      }
    }
    this._J65Hospedeiro_3_5.detectHostChanges(this, this._el_3);
    this._J65Hospedeiro_5_6.detectHostChanges(this, this._el_5);
    this._compView_2.detectChanges();
    if ((!import9.debugThrowIfChanged)) {
      if (firstCheck) {
        this._J65Hospedeiro_3_5.instance.ngAfterViewInit();
        this._J65Todos_0_5.ngAfterViewInit();
      }
      this._J65Todos_0_5.ngAfterViewChecked();
      if (firstCheck) {
        this._J65Hospedeiro_5_6.instance.ngAfterViewInit();
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_4.destroyNestedViews();
    this._compView_2.destroyInternalState();
    this._J65Conteudo_1_5.ngOnDestroy();
    this._J65Filho_2_5.ngOnDestroy();
    this._J65Conteudo_2_6.ngOnDestroy();
    this._J65Todos_0_5.ngOnDestroy();
    this._J65Conteudo_5_5.ngOnDestroy();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J65GanchosDeDiretiva, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J65GanchosDeDiretivaNgFactory = ComponentFactory<import1.J65GanchosDeDiretiva>('j65-ganchos-de-diretiva', viewFactory_J65GanchosDeDiretivaHost0);
ComponentFactory<import1.J65GanchosDeDiretiva> get J65GanchosDeDiretivaNgFactory {
  return _J65GanchosDeDiretivaNgFactory;
}

ComponentFactory<import1.J65GanchosDeDiretiva> createJ65GanchosDeDiretivaFactory() {
  return ComponentFactory('j65-ganchos-de-diretiva', viewFactory_J65GanchosDeDiretivaHost0);
}

class _ViewJ65GanchosDeDiretiva1 extends import15.EmbeddedView<import1.J65GanchosDeDiretiva> {
  late final import1.J65Conteudo _J65Conteudo_0_5;
  late final import1.J65Todos _J65Todos_1_5;
  _ViewJ65GanchosDeDiretiva1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('p'));
    import12.setAttribute(_el_0, 'j65-conteudo', '');
    this._J65Conteudo_0_5 = import1.J65Conteudo();
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_0, this._J65Conteudo_0_5);
    }
    final _el_1 = import12.appendElement<import6.HtmlElement>(doc, _el_0, 'i');
    import12.setAttribute(_el_1, 'j65-todos', '');
    this._J65Todos_1_5 = import1.J65Todos();
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_1, this._J65Todos_1_5);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if ((!import9.debugThrowIfChanged)) {
      if (firstCheck) {
        this._J65Todos_1_5.ngAfterContentInit();
      }
      this._J65Todos_1_5.ngAfterContentChecked();
      if (firstCheck) {
        this._J65Conteudo_0_5.ngAfterContentInit();
      }
    }
    if ((!import9.debugThrowIfChanged)) {
      if (firstCheck) {
        this._J65Todos_1_5.ngAfterViewInit();
      }
      this._J65Todos_1_5.ngAfterViewChecked();
    }
  }

  @override
  void destroyInternal() {
    this._J65Todos_1_5.ngOnDestroy();
    this._J65Conteudo_0_5.ngOnDestroy();
  }
}

import15.EmbeddedView<void> viewFactory_J65GanchosDeDiretiva1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ65GanchosDeDiretiva1(parentView, parentIndex);
}

final List<Object> styles$J65GanchosDeDiretivaHost = const [];

class _ViewJ65GanchosDeDiretivaHost0 extends import8.HostView<import1.J65GanchosDeDiretiva> {
  @override
  void build() {
    this.componentView = ViewJ65GanchosDeDiretiva0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J65GanchosDeDiretiva();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J65GanchosDeDiretiva> viewFactory_J65GanchosDeDiretivaHost0() {
  return _ViewJ65GanchosDeDiretivaHost0();
}

class J65HospedeiroNgCd extends import17.DirectiveChangeDetector {
  final import1.J65Hospedeiro instance;
  Object? _expr_0;
  J65HospedeiroNgCd(this.instance);
  void detectHostChanges(import16.RenderView view, import6.Element el) {
    final currVal_0 = this.instance.ativo;
    if (import9.checkBinding(this._expr_0, currVal_0, null, null)) {
      import12.updateClassBindingNonHtml(el, 'ativo', currVal_0);
      this._expr_0 = currVal_0;
    }
  }
}
