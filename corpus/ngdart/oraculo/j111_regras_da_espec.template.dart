// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j111_regras_da_espec.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j111_regras_da_espec.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/src/runtime/text_binding.dart' as import11;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import14;
import 'package:ngdart/src/runtime/interpolate.dart' as import15;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;

final List<Object> styles$J111Filho = const [];

class ViewJ111Filho0 extends import0.ComponentView<import1.J111Filho> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ111Filho0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j111-filho'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j111_regras_da_espec.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, 'f');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J111Filho, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J111FilhoNgFactory = ComponentFactory<import1.J111Filho>('j111-filho', viewFactory_J111FilhoHost0);
ComponentFactory<import1.J111Filho> get J111FilhoNgFactory {
  return _J111FilhoNgFactory;
}

ComponentFactory<import1.J111Filho> createJ111FilhoFactory() {
  return ComponentFactory('j111-filho', viewFactory_J111FilhoHost0);
}

final List<Object> styles$J111FilhoHost = const [];

class _ViewJ111FilhoHost0 extends import9.HostView<import1.J111Filho> {
  @override
  void build() {
    this.componentView = ViewJ111Filho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J111Filho();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (((!import10.debugThrowIfChanged) && firstCheck)) {
      this.component.ngOnInit();
    }
    this.componentView.detectChanges();
  }
}

import9.HostView<import1.J111Filho> viewFactory_J111FilhoHost0() {
  return _ViewJ111FilhoHost0();
}

final List<Object> styles$J111Usa = const [];

class ViewJ111Usa0 extends import0.ComponentView<import1.J111Usa> {
  final import11.TextBinding _textBinding_7 = import11.TextBinding();
  late final ViewJ111Filho0 _compView_0;
  late final import1.J111Filho _J111Filho_0_5;
  late final ViewContainer _appEl_5;
  late final import1.J111Adiado _J111Adiado_5_9;
  late final import1.J111Fonte _J111Fonte_6_5;
  Object? _expr_1;
  static import2.ComponentStyles? _componentStyles;
  ViewJ111Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j111-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j111_regras_da_espec.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ111Filho0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J111Filho_0_5 = import1.J111Filho();
    this._compView_0.create(this._J111Filho_0_5);
    final doc = import6.document;
    final _el_1 = import7.appendDiv(doc, parentRenderNode);
    final _text_2 = import7.appendText(_el_1, ' ');
    final _el_3 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_4 = import7.appendText(_el_3, ' x');
    final _anchor_5 = import7.appendAnchor(parentRenderNode);
    this._appEl_5 = ViewContainer(5, null, this, _anchor_5);
    var _TemplateRef_5_8 = TemplateRef(this._appEl_5, viewFactory_J111Usa1);
    this._J111Adiado_5_9 = import1.J111Adiado(_TemplateRef_5_8, this._appEl_5);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_5, this._J111Adiado_5_9);
    }
    final _el_6 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'b');
    import7.setAttribute(_el_6, 'j111Fonte', '');
    this._J111Fonte_6_5 = import1.J111Fonte();
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_el_6, this._J111Fonte_6_5);
    }
    _el_6.append(this._textBinding_7.element);
    final subscription_0 = this._J111Fonte_6_5.gatilho.listen(this.eventHandler0(_ctx.ouvir));
    this.initSubscriptions([subscription_0]);
    parentRenderNode.addEventListener('click', this.eventHandler0(_ctx.clicou));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (((!import10.debugThrowIfChanged) && firstCheck)) {
      this._J111Filho_0_5.ngOnInit();
    }
    if (firstCheck) {
      if (import14.isDevToolsEnabled) {
        import14.Inspector.instance.recordInput(this._J111Adiado_5_9, 'j111Adiado', true);
      }
      this._J111Adiado_5_9.preservar = true /* REF:asset:corpus_ngdart/lib/src/j111_regras_da_espec.dart:1083:1106 */;
    }
    final currVal_1 = _ctx.f;
    if (import10.checkBinding(this._expr_1, currVal_1, 'f', 'asset:corpus_ngdart/lib/src/j111_regras_da_espec.dart')) {
      if (import14.isDevToolsEnabled) {
        import14.Inspector.instance.recordInput(this._J111Adiado_5_9, 'j111AdiadoForcar', currVal_1);
      }
      this._J111Adiado_5_9.j111AdiadoForcar = currVal_1 /* REF:asset:corpus_ngdart/lib/src/j111_regras_da_espec.dart:1083:1106 */;
      this._expr_1 = currVal_1;
    }
    this._appEl_5.detectChangesInNestedViews();
    this._textBinding_7.updateText(import15.interpolate0((_ctx.n < 3))) /* REF:asset:corpus_ngdart/lib/src/j111_regras_da_espec.dart:1148:1159 */;
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_5.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J111Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J111UsaNgFactory = ComponentFactory<import1.J111Usa>('j111-usa', viewFactory_J111UsaHost0);
ComponentFactory<import1.J111Usa> get J111UsaNgFactory {
  return _J111UsaNgFactory;
}

ComponentFactory<import1.J111Usa> createJ111UsaFactory() {
  return ComponentFactory('j111-usa', viewFactory_J111UsaHost0);
}

class _ViewJ111Usa1 extends import16.EmbeddedView<import1.J111Usa> {
  _ViewJ111Usa1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('div'));
    final _text_1 = import7.appendText(_el_0, 'a');
    this.initRootNode(_el_0);
  }
}

import16.EmbeddedView<void> viewFactory_J111Usa1(import17.RenderView parentView, int parentIndex) {
  return _ViewJ111Usa1(parentView, parentIndex);
}

final List<Object> styles$J111UsaHost = const [];

class _ViewJ111UsaHost0 extends import9.HostView<import1.J111Usa> {
  @override
  void build() {
    this.componentView = ViewJ111Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J111Usa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J111Usa> viewFactory_J111UsaHost0() {
  return _ViewJ111UsaHost0();
}
