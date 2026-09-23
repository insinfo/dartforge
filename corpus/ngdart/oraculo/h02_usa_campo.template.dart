// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'h02_usa_campo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'h02_usa_campo.dart' as import1;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import2;
import 'h02_campo.template.dart' as import3;
import 'h02_campo.dart' as import4;
import 'package:ngforms/src/directives/ng_model.dart' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/meta/di_tokens.dart' as import13;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import14;
import 'package:ngforms/src/directives/ng_control.dart' as import15;
import 'package:ngdart/src/runtime/check_binding.dart' as import16;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import18;

final List<Object> styles$H02UsaCampo = const [];

class ViewH02UsaCampo0 extends import0.ComponentView<import1.H02UsaCampo> {
  late List<import2.ControlValueAccessor<dynamic>> _NgValueAccessor_2_6 = [this._H02Campo_2_5];
  late final import3.ViewH02Campo0 _compView_1;
  late final import4.H02Campo _H02Campo_1_5;
  late final List<import2.ControlValueAccessor<dynamic>> _NgValueAccessor_1_6;
  late final import5.NgModel _NgModel_1_7;
  late final import3.ViewH02Campo0 _compView_2;
  late final import4.H02Campo _H02Campo_2_5;
  Object? _expr_1;
  static import6.ComponentStyles? _componentStyles;
  ViewH02UsaCampo0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('h02-usa-campo'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/h02_usa_campo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import10.document;
    final _el_0 = import11.appendDiv(doc, parentRenderNode);
    this._compView_1 = import3.ViewH02Campo0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    import11.setAttribute(_el_1, 'rotulo', 'Com');
    this._H02Campo_1_5 = import4.H02Campo();
    this._NgValueAccessor_1_6 = [this._H02Campo_1_5];
    this._NgModel_1_7 = import5.NgModel(null, this._NgValueAccessor_1_6);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_1, this._NgModel_1_7);
    }
    this._compView_1.create(this._H02Campo_1_5);
    this._compView_2 = import3.ViewH02Campo0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    import11.setAttribute(_el_2, 'rotulo', 'Sem');
    this._H02Campo_2_5 = import4.H02Campo();
    this._compView_2.create(this._H02Campo_2_5);
    final subscription_0 = this._NgModel_1_7.update.listen(this.eventHandler1(this._handleEvent_0));
    this.initSubscriptions([subscription_0]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((1 == nodeIndex)) {
      if (identical(token, const import13.MultiToken<import14.ControlValueAccessor<dynamic>>('NgValueAccessor'))) {
        return this._NgValueAccessor_1_6;
      }
      if ((identical(token, import5.NgModel) || identical(token, import15.NgControl))) {
        return this._NgModel_1_7;
      }
    }
    if ((identical(token, const import13.MultiToken<import14.ControlValueAccessor<dynamic>>('NgValueAccessor')) && (2 == nodeIndex))) {
      return this._NgValueAccessor_2_6;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool changed = false;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._H02Campo_1_5, 'rotulo', 'Com');
      }
      this._H02Campo_1_5.rotulo = 'Com' /* REF:package:corpus_ngdart/src/h02_usa_campo.html:16:28 */;
    }
    changed = false;
    final currVal_1 = _ctx.valor;
    if (import16.checkBinding(this._expr_1, currVal_1, 'valor', 'package:corpus_ngdart/src/h02_usa_campo.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgModel_1_7, 'ngModel', currVal_1);
      }
      this._NgModel_1_7.model = currVal_1 /* REF:package:corpus_ngdart/src/h02_usa_campo.html:29:48 */;
      changed = true;
      this._expr_1 = currVal_1;
    }
    if (changed) {
      this._NgModel_1_7.ngAfterChanges();
    }
    if (((!import16.debugThrowIfChanged) && firstCheck)) {
      this._NgModel_1_7.ngOnInit();
    }
    if (firstCheck) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._H02Campo_2_5, 'rotulo', 'Sem');
      }
      this._H02Campo_2_5.rotulo = 'Sem' /* REF:package:corpus_ngdart/src/h02_usa_campo.html:79:91 */;
    }
    this._compView_1.detectChanges();
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
    this._compView_2.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.valor = $event;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$H02UsaCampo, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _H02UsaCampoNgFactory = ComponentFactory<import1.H02UsaCampo>('h02-usa-campo', viewFactory_H02UsaCampoHost0);
ComponentFactory<import1.H02UsaCampo> get H02UsaCampoNgFactory {
  return _H02UsaCampoNgFactory;
}

ComponentFactory<import1.H02UsaCampo> createH02UsaCampoFactory() {
  return ComponentFactory('h02-usa-campo', viewFactory_H02UsaCampoHost0);
}

final List<Object> styles$H02UsaCampoHost = const [];

class _ViewH02UsaCampoHost0 extends import18.HostView<import1.H02UsaCampo> {
  @override
  void build() {
    this.componentView = ViewH02UsaCampo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.H02UsaCampo();
    this.initRootNode(_el_0);
  }
}

import18.HostView<import1.H02UsaCampo> viewFactory_H02UsaCampoHost0() {
  return _ViewH02UsaCampoHost0();
}
