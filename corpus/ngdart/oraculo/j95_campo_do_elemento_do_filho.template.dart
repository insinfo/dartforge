// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j95_campo_do_elemento_do_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j95_campo_do_elemento_do_filho.dart' as import1;
import 'j92_filho_com_ligacoes_do_elemento.template.dart' as import2;
import 'j92_filho_com_ligacoes_do_elemento.dart' as import3;
import 'dart:html' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/src/runtime/check_binding.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import13;

final List<Object> styles$J95CampoDoElementoDoFilho = const [];

class ViewJ95CampoDoElementoDoFilho0 extends import0.ComponentView<import1.J95CampoDoElementoDoFilho> {
  late final import2.ViewJ92Filho0 _compView_0;
  late final import3.J92Filho _J92Filho_0_5;
  late final import2.J92DestaqueNgCd _J92Destaque_0_6;
  Object? _expr_0;
  late final import4.HtmlElement _el_0;
  static import5.ComponentStyles? _componentStyles;
  ViewJ95CampoDoElementoDoFilho0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import4.document.createElement('j95-campo-do-elemento-do-filho'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/j95_campo_do_elemento_do_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewJ92Filho0(this, 0);
    this._el_0 = this._compView_0.rootElement;
    parentRenderNode.append(this._el_0);
    import9.setAttribute(this._el_0, 'j92-destaque', '');
    this._J92Filho_0_5 = import3.J92Filho();
    this._J92Destaque_0_6 = import2.J92DestaqueNgCd(import3.J92Destaque());
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(this._el_0, this._J92Destaque_0_6.instance);
    }
    this._compView_0.create(this._J92Filho_0_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.ativo;
    if (import11.checkBinding(this._expr_0, currVal_0, 'ativo', 'asset:corpus_ngdart/lib/src/j95_campo_do_elemento_do_filho.dart')) {
      import9.updateClassBindingNonHtml(this._el_0, 'ativo', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j95_campo_do_elemento_do_filho.dart:367:388 */;
      this._expr_0 = currVal_0;
    }
    this._J92Destaque_0_6.detectHostChanges(this._compView_0, this._el_0);
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$J95CampoDoElementoDoFilho, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J95CampoDoElementoDoFilhoNgFactory = ComponentFactory<import1.J95CampoDoElementoDoFilho>('j95-campo-do-elemento-do-filho', viewFactory_J95CampoDoElementoDoFilhoHost0);
ComponentFactory<import1.J95CampoDoElementoDoFilho> get J95CampoDoElementoDoFilhoNgFactory {
  return _J95CampoDoElementoDoFilhoNgFactory;
}

ComponentFactory<import1.J95CampoDoElementoDoFilho> createJ95CampoDoElementoDoFilhoFactory() {
  return ComponentFactory('j95-campo-do-elemento-do-filho', viewFactory_J95CampoDoElementoDoFilhoHost0);
}

final List<Object> styles$J95CampoDoElementoDoFilhoHost = const [];

class _ViewJ95CampoDoElementoDoFilhoHost0 extends import13.HostView<import1.J95CampoDoElementoDoFilho> {
  @override
  void build() {
    this.componentView = ViewJ95CampoDoElementoDoFilho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J95CampoDoElementoDoFilho();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.J95CampoDoElementoDoFilho> viewFactory_J95CampoDoElementoDoFilhoHost0() {
  return _ViewJ95CampoDoElementoDoFilhoHost0();
}
