// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j42_atributo_injetado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j42_atributo_injetado.dart' as import1;
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
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import13;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import14;
import 'package:ngdart/src/runtime/check_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$J42AtributoInjetado = const [];

class ViewJ42AtributoInjetado0 extends import0.ComponentView<import1.J42AtributoInjetado> {
  late final import1.J42Alvo _J42Alvo_0_5;
  late final import1.J42Alvo _J42Alvo_3_5;
  late final ViewContainer _appEl_5;
  late final NgIf _NgIf_5_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ42AtributoInjetado0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j42-atributo-injetado'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j42_atributo_injetado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.AnchorElement>(doc, parentRenderNode, 'a');
    import9.setAttribute(_el_0, 'j42-alvo', '');
    import9.setAttribute(_el_0, 'rel', 'noopener');
    import9.setAttribute(_el_0, 'target', '_blank');
    this._J42Alvo_0_5 = import1.J42Alvo('_blank', 'noopener', null);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_0, this._J42Alvo_0_5);
    }
    final _text_1 = import9.appendText(_el_0, 'a');
    final _text_2 = import9.appendText(parentRenderNode, '\n');
    final _el_3 = import9.appendElement<import8.AnchorElement>(doc, parentRenderNode, 'a');
    import9.setAttribute(_el_3, 'data-marca', '');
    import9.setAttribute(_el_3, 'j42-alvo', '');
    this._J42Alvo_3_5 = import1.J42Alvo(null, null, '');
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_3, this._J42Alvo_3_5);
    }
    final _text_4 = import9.appendText(_el_3, 'b');
    final _anchor_5 = import9.appendAnchor(parentRenderNode);
    this._appEl_5 = ViewContainer(5, null, this, _anchor_5);
    var _TemplateRef_5_8 = TemplateRef(this._appEl_5, viewFactory_J42AtributoInjetado1);
    this._NgIf_5_9 = NgIf(this._appEl_5, _TemplateRef_5_8);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_anchor_5, this._NgIf_5_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.recordInput(this._NgIf_5_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_5_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j42_atributo_injetado.html:81:96 */;
    this._appEl_5.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_5.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J42AtributoInjetado, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J42AtributoInjetadoNgFactory = ComponentFactory<import1.J42AtributoInjetado>('j42-atributo-injetado', viewFactory_J42AtributoInjetadoHost0);
ComponentFactory<import1.J42AtributoInjetado> get J42AtributoInjetadoNgFactory {
  return _J42AtributoInjetadoNgFactory;
}

ComponentFactory<import1.J42AtributoInjetado> createJ42AtributoInjetadoFactory() {
  return ComponentFactory('j42-atributo-injetado', viewFactory_J42AtributoInjetadoHost0);
}

class _ViewJ42AtributoInjetado1 extends import13.EmbeddedView<import1.J42AtributoInjetado> {
  late final import1.J42Alvo _J42Alvo_1_5;
  Object? _expr_0;
  late final import8.AnchorElement _el_1;
  _ViewJ42AtributoInjetado1(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    this._el_1 = import9.appendElement<import8.AnchorElement>(doc, _el_0, 'a');
    import9.setAttribute(this._el_1, 'j42-alvo', '');
    import9.setAttribute(this._el_1, 'rel', 'it&apos;s');
    this._J42Alvo_1_5 = import1.J42Alvo(null, 'it&apos;s', null);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(this._el_1, this._J42Alvo_1_5);
    }
    final _text_2 = import9.appendText(this._el_1, 'c');
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.destino;
    if (import15.checkBinding(this._expr_0, currVal_0, 'destino', 'package:corpus_ngdart/src/j42_atributo_injetado.html')) {
      import9.updateAttribute(this._el_1, 'target', currVal_0) /* REF:package:corpus_ngdart/src/j42_atributo_injetado.html:109:132 */;
      this._expr_0 = currVal_0;
    }
  }
}

import13.EmbeddedView<void> viewFactory_J42AtributoInjetado1(import14.RenderView parentView, int parentIndex) {
  return _ViewJ42AtributoInjetado1(parentView, parentIndex);
}

final List<Object> styles$J42AtributoInjetadoHost = const [];

class _ViewJ42AtributoInjetadoHost0 extends import16.HostView<import1.J42AtributoInjetado> {
  @override
  void build() {
    this.componentView = ViewJ42AtributoInjetado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J42AtributoInjetado();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.J42AtributoInjetado> viewFactory_J42AtributoInjetadoHost0() {
  return _ViewJ42AtributoInjetadoHost0();
}
