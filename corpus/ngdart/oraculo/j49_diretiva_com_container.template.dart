// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j49_diretiva_com_container.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j49_diretiva_com_container.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$J49DiretivaComContainer = const [];

class ViewJ49DiretivaComContainer0 extends import0.ComponentView<import1.J49DiretivaComContainer> {
  late final ViewContainer _appEl_0;
  late final import1.J49Saida _J49Saida_0_8;
  late final ViewContainer _appEl_3;
  late final import1.J49Saida _J49Saida_3_8;
  late final ViewContainer _appEl_5;
  late final NgIf _NgIf_5_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewJ49DiretivaComContainer0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j49-diretiva-com-container'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j49_diretiva_com_container.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendDiv(doc, parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _el_0);
    this._J49Saida_0_8 = import1.J49Saida(_el_0, this._appEl_0);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_0, this._J49Saida_0_8);
    }
    final _text_1 = import9.appendText(_el_0, 'a');
    final _el_2 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'section');
    final _el_3 = import9.appendSpan(doc, _el_2);
    import9.setAttribute(_el_3, 'j49-saida', '');
    this._appEl_3 = ViewContainer(3, 2, this, _el_3);
    this._J49Saida_3_8 = import1.J49Saida(_el_3, this._appEl_3);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_3, this._J49Saida_3_8);
    }
    final _text_4 = import9.appendText(_el_3, 'b');
    final _anchor_5 = import9.appendAnchor(parentRenderNode);
    this._appEl_5 = ViewContainer(5, null, this, _anchor_5);
    var _TemplateRef_5_8 = TemplateRef(this._appEl_5, viewFactory_J49DiretivaComContainer1);
    this._NgIf_5_9 = NgIf(this._appEl_5, _TemplateRef_5_8);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_anchor_5, this._NgIf_5_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final currVal_0 = _ctx.v;
    if (import12.checkBinding(this._expr_0, currVal_0, 'v', 'package:corpus_ngdart/src/j49_diretiva_com_container.html')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J49Saida_0_8, 'j49-saida', currVal_0);
      }
      this._J49Saida_0_8.valor = currVal_0 /* REF:package:corpus_ngdart/src/j49_diretiva_com_container.html:5:20 */;
      this._expr_0 = currVal_0;
    }
    if (firstCheck) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J49Saida_3_8, 'j49-saida', '');
      }
      this._J49Saida_3_8.valor = '' /* REF:package:corpus_ngdart/src/j49_diretiva_com_container.html:44:53 */;
    }
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.recordInput(this._NgIf_5_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_5_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j49_diretiva_com_container.html:76:91 */;
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_3.detectChangesInNestedViews();
    this._appEl_5.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_3.destroyNestedViews();
    this._appEl_5.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J49DiretivaComContainer, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J49DiretivaComContainerNgFactory = ComponentFactory<import1.J49DiretivaComContainer>('j49-diretiva-com-container', viewFactory_J49DiretivaComContainerHost0);
ComponentFactory<import1.J49DiretivaComContainer> get J49DiretivaComContainerNgFactory {
  return _J49DiretivaComContainerNgFactory;
}

ComponentFactory<import1.J49DiretivaComContainer> createJ49DiretivaComContainerFactory() {
  return ComponentFactory('j49-diretiva-com-container', viewFactory_J49DiretivaComContainerHost0);
}

class _ViewJ49DiretivaComContainer1 extends import14.EmbeddedView<import1.J49DiretivaComContainer> {
  late final ViewContainer _appEl_1;
  late final import1.J49Saida _J49Saida_1_8;
  Object? _expr_0;
  _ViewJ49DiretivaComContainer1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _el_1 = import9.appendElement<import8.HtmlElement>(doc, _el_0, 'i');
    this._appEl_1 = ViewContainer(1, 0, this, _el_1);
    this._J49Saida_1_8 = import1.J49Saida(_el_1, this._appEl_1);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_1, this._J49Saida_1_8);
    }
    final _text_2 = import9.appendText(_el_1, 'c');
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.v;
    if (import12.checkBinding(this._expr_0, currVal_0, 'v', 'package:corpus_ngdart/src/j49_diretiva_com_container.html')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J49Saida_1_8, 'j49-saida', currVal_0);
      }
      this._J49Saida_1_8.valor = currVal_0 /* REF:package:corpus_ngdart/src/j49_diretiva_com_container.html:95:110 */;
      this._expr_0 = currVal_0;
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import14.EmbeddedView<void> viewFactory_J49DiretivaComContainer1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ49DiretivaComContainer1(parentView, parentIndex);
}

final List<Object> styles$J49DiretivaComContainerHost = const [];

class _ViewJ49DiretivaComContainerHost0 extends import16.HostView<import1.J49DiretivaComContainer> {
  @override
  void build() {
    this.componentView = ViewJ49DiretivaComContainer0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J49DiretivaComContainer();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.J49DiretivaComContainer> viewFactory_J49DiretivaComContainerHost0() {
  return _ViewJ49DiretivaComContainerHost0();
}
