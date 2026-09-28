// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j85_diretivas_de_template.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j85_diretivas_de_template.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/runtime/text_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import18;

final List<Object> styles$J85DiretivasDeTemplate = const [];

class ViewJ85DiretivasDeTemplate0 extends import0.ComponentView<import1.J85DiretivasDeTemplate> {
  late final import1.J85Grupo _J85Grupo_0_5;
  late final ViewContainer _appEl_1;
  late final J85MarcaNgCd _J85Marca_1_9;
  late final import1.J85Saida _J85Saida_1_10;
  late final ViewContainer _appEl_3;
  late final TemplateRef _TemplateRef_3_7;
  late final import1.J85Simples _J85Simples_3_8;
  late final ViewContainer _appEl_4;
  late final NgIf _NgIf_4_9;
  Object? _expr_1;
  static import5.ComponentStyles? _componentStyles;
  ViewJ85DiretivasDeTemplate0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('j85-diretivas-de-template'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/j85_diretivas_de_template.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import9.document;
    final _el_0 = import10.appendDiv(doc, parentRenderNode);
    import10.setAttribute(_el_0, 'j85-grupo', '');
    this._J85Grupo_0_5 = import1.J85Grupo();
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_0, this._J85Grupo_0_5);
    }
    final _anchor_1 = import10.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J85DiretivasDeTemplate1);
    this._J85Marca_1_9 = J85MarcaNgCd(import1.J85Marca(this._J85Grupo_0_5));
    this._J85Saida_1_10 = import1.J85Saida(_TemplateRef_1_8, this._appEl_1, this._J85Grupo_0_5);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._J85Marca_1_9.instance);
      import11.Inspector.instance.registerDirective(_anchor_1, this._J85Saida_1_10);
    }
    final _text_2 = import10.appendText(_el_0, ' ');
    final _anchor_3 = import10.appendAnchor(_el_0);
    this._appEl_3 = ViewContainer(3, 0, this, _anchor_3);
    this._TemplateRef_3_7 = TemplateRef(this._appEl_3, viewFactory_J85DiretivasDeTemplate2);
    this._J85Simples_3_8 = import1.J85Simples(this._TemplateRef_3_7);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_3, this._J85Simples_3_8);
    }
    final _anchor_4 = import10.appendAnchor(_el_0);
    this._appEl_4 = ViewContainer(4, 0, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_J85DiretivasDeTemplate3);
    this._NgIf_4_9 = NgIf(this._appEl_4, _TemplateRef_4_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_4, this._NgIf_4_9);
    }
    final subscription_0 = this._J85Saida_1_10.pronto.listen(this.eventHandler1(_ctx.avisar));
    this.initSubscriptions([subscription_0]);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool changed = false;
    bool firstCheck = this.firstCheck;
    changed = false;
    if (firstCheck) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._J85Saida_1_10, 'j85-saida', 'a');
      }
      this._J85Saida_1_10.nome = 'a' /* REF:package:corpus_ngdart/src/j85_diretivas_de_template.html:28:41 */;
      changed = true;
    }
    final currVal_1 = _ctx.total;
    if (import12.checkBinding(this._expr_1, currVal_1, 'total', 'package:corpus_ngdart/src/j85_diretivas_de_template.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._J85Saida_1_10, 'valor', currVal_1);
      }
      this._J85Saida_1_10.valor = currVal_1 /* REF:package:corpus_ngdart/src/j85_diretivas_de_template.html:42:57 */;
      changed = true;
      this._expr_1 = currVal_1;
    }
    if (changed) {
      this._J85Saida_1_10.ngAfterChanges();
    }
    if (((!import12.debugThrowIfChanged) && firstCheck)) {
      this._J85Saida_1_10.ngOnInit();
    }
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_4_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_4_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j85_diretivas_de_template.html:179:194 */;
    this._appEl_1.detectChangesInNestedViews();
    this._appEl_4.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._appEl_4.destroyNestedViews();
    this._J85Saida_1_10.ngOnDestroy();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$J85DiretivasDeTemplate, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J85DiretivasDeTemplateNgFactory = ComponentFactory<import1.J85DiretivasDeTemplate>('j85-diretivas-de-template', viewFactory_J85DiretivasDeTemplateHost0);
ComponentFactory<import1.J85DiretivasDeTemplate> get J85DiretivasDeTemplateNgFactory {
  return _J85DiretivasDeTemplateNgFactory;
}

ComponentFactory<import1.J85DiretivasDeTemplate> createJ85DiretivasDeTemplateFactory() {
  return ComponentFactory('j85-diretivas-de-template', viewFactory_J85DiretivasDeTemplateHost0);
}

class _ViewJ85DiretivasDeTemplate1 extends import14.EmbeddedView<import1.J85DiretivasDeTemplate> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  _ViewJ85DiretivasDeTemplate1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('b'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateTextWithPrimitive(_ctx.total) /* REF:package:corpus_ngdart/src/j85_diretivas_de_template.html:92:103 */;
  }
}

import14.EmbeddedView<void> viewFactory_J85DiretivasDeTemplate1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ85DiretivasDeTemplate1(parentView, parentIndex);
}

class _ViewJ85DiretivasDeTemplate2 extends import14.EmbeddedView<import1.J85DiretivasDeTemplate> {
  _ViewJ85DiretivasDeTemplate2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('i'));
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_J85DiretivasDeTemplate2(import16.RenderView parentView, int parentIndex) {
  return _ViewJ85DiretivasDeTemplate2(parentView, parentIndex);
}

class _ViewJ85DiretivasDeTemplate3 extends import14.EmbeddedView<import1.J85DiretivasDeTemplate> {
  late final ViewContainer _appEl_1;
  late final J85MarcaNgCd _J85Marca_1_9;
  late final import1.J85Saida _J85Saida_1_10;
  _ViewJ85DiretivasDeTemplate3(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('p'));
    final _anchor_1 = import10.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J85DiretivasDeTemplate4);
    this._J85Marca_1_9 = J85MarcaNgCd(import1.J85Marca(import8.unsafeCast<ViewJ85DiretivasDeTemplate0>((this.parentView!))._J85Grupo_0_5));
    this._J85Saida_1_10 = import1.J85Saida(_TemplateRef_1_8, this._appEl_1, import8.unsafeCast<ViewJ85DiretivasDeTemplate0>((this.parentView!))._J85Grupo_0_5);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._J85Marca_1_9.instance);
      import11.Inspector.instance.registerDirective(_anchor_1, this._J85Saida_1_10);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool changed = false;
    bool firstCheck = this.firstCheck;
    changed = false;
    if (firstCheck) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._J85Saida_1_10, 'j85-saida', 'b');
      }
      this._J85Saida_1_10.nome = 'b' /* REF:package:corpus_ngdart/src/j85_diretivas_de_template.html:210:223 */;
      changed = true;
    }
    if (changed) {
      this._J85Saida_1_10.ngAfterChanges();
    }
    if (((!import12.debugThrowIfChanged) && firstCheck)) {
      this._J85Saida_1_10.ngOnInit();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._J85Saida_1_10.ngOnDestroy();
  }
}

import14.EmbeddedView<void> viewFactory_J85DiretivasDeTemplate3(import16.RenderView parentView, int parentIndex) {
  return _ViewJ85DiretivasDeTemplate3(parentView, parentIndex);
}

class _ViewJ85DiretivasDeTemplate4 extends import14.EmbeddedView<import1.J85DiretivasDeTemplate> {
  _ViewJ85DiretivasDeTemplate4(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('span'));
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_J85DiretivasDeTemplate4(import16.RenderView parentView, int parentIndex) {
  return _ViewJ85DiretivasDeTemplate4(parentView, parentIndex);
}

final List<Object> styles$J85DiretivasDeTemplateHost = const [];

class _ViewJ85DiretivasDeTemplateHost0 extends import17.HostView<import1.J85DiretivasDeTemplate> {
  @override
  void build() {
    this.componentView = ViewJ85DiretivasDeTemplate0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J85DiretivasDeTemplate();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.J85DiretivasDeTemplate> viewFactory_J85DiretivasDeTemplateHost0() {
  return _ViewJ85DiretivasDeTemplateHost0();
}

class J85MarcaNgCd extends import18.DirectiveChangeDetector {
  final import1.J85Marca instance;
  Object? _expr_0;
  J85MarcaNgCd(this.instance);
  void detectHostChanges(import16.RenderView view, import9.Element el) {
    final currVal_0 = this.instance.marca;
    if (import12.checkBinding(this._expr_0, currVal_0, null, null)) {
      import10.updateClassBindingNonHtml(el, 'marca', currVal_0);
      this._expr_0 = currVal_0;
    }
  }
}
