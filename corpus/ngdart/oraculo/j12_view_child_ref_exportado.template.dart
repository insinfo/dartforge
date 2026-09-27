// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j12_view_child_ref_exportado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j12_view_child_ref_exportado.dart' as import1;
import 'package:ngforms/src/directives/ng_form.dart' as import2;
import 'package:ngforms/src/directives/default_value_accessor.dart' as import3;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import4;
import 'package:ngforms/src/directives/ng_control_name.dart' as import5;
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
import 'package:ngforms/src/directives/control_container.dart' as import16;
import 'package:ngdart/src/runtime/check_binding.dart' as import17;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$J12ViewChildRefExportado = const [];

class ViewJ12ViewChildRefExportado0 extends import0.ComponentView<import1.J12ViewChildRefExportado> {
  late final import2.NgForm _NgForm_0_5;
  late final import3.DefaultValueAccessor _DefaultValueAccessor_1_5;
  late final List<import4.ControlValueAccessor<dynamic>> _NgValueAccessor_1_6;
  late final import5.NgControlName _NgControlName_1_7;
  Object? _expr_0;
  static import6.ComponentStyles? _componentStyles;
  ViewJ12ViewChildRefExportado0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('j12-view-child-ref-exportado'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/j12_view_child_ref_exportado.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import10.document;
    final _el_0 = import11.appendElement<import10.FormElement>(doc, parentRenderNode, 'form');
    this._NgForm_0_5 = import2.NgForm(null, this);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_0, this._NgForm_0_5);
    }
    final _el_1 = import11.appendElement<import10.InputElement>(doc, _el_0, 'input');
    import11.setAttribute(_el_1, 'ngControl', 'nome');
    this._DefaultValueAccessor_1_5 = import3.DefaultValueAccessor(_el_1);
    this._NgValueAccessor_1_6 = [this._DefaultValueAccessor_1_5];
    this._NgControlName_1_7 = import5.NgControlName(this._NgForm_0_5, null, this._NgValueAccessor_1_6);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_1, this._DefaultValueAccessor_1_5);
      import12.Inspector.instance.registerDirective(_el_1, this._NgControlName_1_7);
    }
    _el_0.addEventListener('submit', this.eventHandler1(this._NgForm_0_5.onSubmit));
    _el_0.addEventListener('reset', this.eventHandler1(this._NgForm_0_5.onReset));
    _el_1.addEventListener('blur', this.eventHandler0(this._DefaultValueAccessor_1_5.touchHandler));
    _el_1.addEventListener('input', this.eventHandler1(this._handleEvent_0));
    final subscription_0 = this._NgControlName_1_7.update.listen(this.eventHandler1(this._handleEvent_1));
    _ctx.form = this._NgForm_0_5;
    this.initSubscriptions([subscription_0]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((nodeIndex <= 1)) {
      if ((1 == nodeIndex)) {
        if (identical(token, const import13.MultiToken<import14.ControlValueAccessor<dynamic>>('NgValueAccessor'))) {
          return this._NgValueAccessor_1_6;
        }
        if (identical(token, import15.NgControl)) {
          return this._NgControlName_1_7;
        }
      }
      if ((identical(token, import2.NgForm) || identical(token, import16.ControlContainer))) {
        return this._NgForm_0_5;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool changed = false;
    bool firstCheck = this.firstCheck;
    changed = false;
    if (firstCheck) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgControlName_1_7, 'ngControl', 'nome');
      }
      this._NgControlName_1_7.name = 'nome' /* REF:package:corpus_ngdart/src/j12_view_child_ref_exportado.html:44:60 */;
      changed = true;
    }
    final currVal_0 = _ctx.nome;
    if (import17.checkBinding(this._expr_0, currVal_0, 'nome', 'package:corpus_ngdart/src/j12_view_child_ref_exportado.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgControlName_1_7, 'ngModel', currVal_0);
      }
      this._NgControlName_1_7.model = currVal_0 /* REF:package:corpus_ngdart/src/j12_view_child_ref_exportado.html:25:43 */;
      changed = true;
      this._expr_0 = currVal_0;
    }
    if (changed) {
      this._NgControlName_1_7.ngAfterChanges();
    }
  }

  @override
  void destroyInternal() {
    this._NgControlName_1_7.ngOnDestroy();
  }

  void _handleEvent_0($event) {
    this._DefaultValueAccessor_1_5.handleChange($event.target.value);
  }

  void _handleEvent_1($event) {
    final _ctx = this.ctx;
    _ctx.nome = $event;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J12ViewChildRefExportado, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J12ViewChildRefExportadoNgFactory = ComponentFactory<import1.J12ViewChildRefExportado>('j12-view-child-ref-exportado', viewFactory_J12ViewChildRefExportadoHost0);
ComponentFactory<import1.J12ViewChildRefExportado> get J12ViewChildRefExportadoNgFactory {
  return _J12ViewChildRefExportadoNgFactory;
}

ComponentFactory<import1.J12ViewChildRefExportado> createJ12ViewChildRefExportadoFactory() {
  return ComponentFactory('j12-view-child-ref-exportado', viewFactory_J12ViewChildRefExportadoHost0);
}

final List<Object> styles$J12ViewChildRefExportadoHost = const [];

class _ViewJ12ViewChildRefExportadoHost0 extends import19.HostView<import1.J12ViewChildRefExportado> {
  @override
  void build() {
    this.componentView = ViewJ12ViewChildRefExportado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J12ViewChildRefExportado();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.J12ViewChildRefExportado> viewFactory_J12ViewChildRefExportadoHost0() {
  return _ViewJ12ViewChildRefExportadoHost0();
}
