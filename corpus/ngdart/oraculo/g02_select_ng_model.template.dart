// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'g02_select_ng_model.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'g02_select_ng_model.dart' as import1;
import 'package:ngforms/src/directives/select_control_value_accessor.dart' as import2;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import3;
import 'package:ngforms/src/directives/ng_model.dart' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/meta/di_tokens.dart' as import12;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import13;
import 'package:ngforms/src/directives/ng_control.dart' as import14;
import 'package:ngdart/src/runtime/check_binding.dart' as import15;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$G02SelectNgModel = const [];

class ViewG02SelectNgModel0 extends import0.ComponentView<import1.G02SelectNgModel> {
  late final import2.SelectControlValueAccessor _SelectControlValueAccessor_0_5;
  late final List<import3.ControlValueAccessor<dynamic>> _NgValueAccessor_0_6;
  late final import4.NgModel _NgModel_0_7;
  late final import2.NgSelectOption _NgSelectOption_1_5;
  Object? _expr_0;
  static import5.ComponentStyles? _componentStyles;
  ViewG02SelectNgModel0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('g02-select-ng-model'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/g02_select_ng_model.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import9.document;
    final _el_0 = import10.appendElement<import9.SelectElement>(doc, parentRenderNode, 'select');
    this._SelectControlValueAccessor_0_5 = import2.SelectControlValueAccessor(_el_0);
    this._NgValueAccessor_0_6 = [this._SelectControlValueAccessor_0_5];
    this._NgModel_0_7 = import4.NgModel(null, this._NgValueAccessor_0_6);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_0, this._SelectControlValueAccessor_0_5);
      import11.Inspector.instance.registerDirective(_el_0, this._NgModel_0_7);
    }
    final _el_1 = import10.appendElement<import9.OptionElement>(doc, _el_0, 'option');
    import10.setAttribute(_el_1, 'value', 'a');
    this._NgSelectOption_1_5 = import2.NgSelectOption(_el_1, this._SelectControlValueAccessor_0_5);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_1, this._NgSelectOption_1_5);
    }
    final _text_2 = import10.appendText(_el_1, 'A');
    _el_0.addEventListener('blur', this.eventHandler0(this._SelectControlValueAccessor_0_5.touchHandler));
    _el_0.addEventListener('change', this.eventHandler1(this._handleEvent_0));
    final subscription_0 = this._NgModel_0_7.update.listen(this.eventHandler1(this._handleEvent_1));
    this.initSubscriptions([subscription_0]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((nodeIndex <= 2)) {
      if (identical(token, import2.SelectControlValueAccessor)) {
        return this._SelectControlValueAccessor_0_5;
      }
      if (identical(token, const import12.MultiToken<import13.ControlValueAccessor<dynamic>>('NgValueAccessor'))) {
        return this._NgValueAccessor_0_6;
      }
      if ((identical(token, import4.NgModel) || identical(token, import14.NgControl))) {
        return this._NgModel_0_7;
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
    final currVal_0 = _ctx.escolha;
    if (import15.checkBinding(this._expr_0, currVal_0, 'escolha', 'package:corpus_ngdart/src/g02_select_ng_model.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgModel_0_7, 'ngModel', currVal_0);
      }
      this._NgModel_0_7.model = currVal_0 /* REF:package:corpus_ngdart/src/g02_select_ng_model.html:8:29 */;
      changed = true;
      this._expr_0 = currVal_0;
    }
    if (changed) {
      this._NgModel_0_7.ngAfterChanges();
    }
    if (((!import15.debugThrowIfChanged) && firstCheck)) {
      this._NgModel_0_7.ngOnInit();
    }
    if (firstCheck) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgSelectOption_1_5, 'value', 'a');
      }
      this._NgSelectOption_1_5.value = 'a' /* REF:package:corpus_ngdart/src/g02_select_ng_model.html:41:50 */;
    }
  }

  @override
  void destroyInternal() {
    this._NgSelectOption_1_5.ngOnDestroy();
  }

  void _handleEvent_0($event) {
    this._SelectControlValueAccessor_0_5.handleChange($event.target.value);
  }

  void _handleEvent_1($event) {
    final _ctx = this.ctx;
    _ctx.escolha = $event;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$G02SelectNgModel, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _G02SelectNgModelNgFactory = ComponentFactory<import1.G02SelectNgModel>('g02-select-ng-model', viewFactory_G02SelectNgModelHost0);
ComponentFactory<import1.G02SelectNgModel> get G02SelectNgModelNgFactory {
  return _G02SelectNgModelNgFactory;
}

ComponentFactory<import1.G02SelectNgModel> createG02SelectNgModelFactory() {
  return ComponentFactory('g02-select-ng-model', viewFactory_G02SelectNgModelHost0);
}

final List<Object> styles$G02SelectNgModelHost = const [];

class _ViewG02SelectNgModelHost0 extends import17.HostView<import1.G02SelectNgModel> {
  @override
  void build() {
    this.componentView = ViewG02SelectNgModel0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.G02SelectNgModel();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.G02SelectNgModel> viewFactory_G02SelectNgModelHost0() {
  return _ViewG02SelectNgModelHost0();
}
