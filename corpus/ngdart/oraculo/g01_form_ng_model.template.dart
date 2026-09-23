// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'g01_form_ng_model.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'g01_form_ng_model.dart' as import1;
import 'package:ngforms/src/directives/ng_form.dart' as import2;
import 'package:ngforms/src/directives/validators.dart' as import3;
import 'dart:core';
import 'package:ngforms/src/directives/default_value_accessor.dart' as import5;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import6;
import 'package:ngforms/src/directives/ng_model.dart' as import7;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'dart:html' as import10;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import11;
import 'package:ngdart/src/core/linker/views/view.dart' as import12;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import13;
import 'package:ngdart/src/utilities.dart' as import14;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import15;
import 'package:ngdart/src/devtools.dart' as import16;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/meta/di_tokens.dart' as import18;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import19;
import 'package:ngforms/src/directives/ng_control.dart' as import20;
import 'package:ngforms/src/directives/control_container.dart' as import21;
import 'package:ngdart/src/runtime/check_binding.dart' as import22;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import24;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import25;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import26;

final List<Object> styles$G01FormNgModel = const [];

class ViewG01FormNgModel0 extends import0.ComponentView<import1.G01FormNgModel> {
  late final import2.NgForm _NgForm_0_5;
  late final import3.RequiredValidator _RequiredValidator_1_5;
  late final List<Object> _NgValidators_1_6;
  late final import5.DefaultValueAccessor _DefaultValueAccessor_1_7;
  late final List<import6.ControlValueAccessor<dynamic>> _NgValueAccessor_1_8;
  late final import7.NgModel _NgModel_1_9;
  late final import3.RequiredValidator _RequiredValidator_3_5;
  late final List<Object> _NgValidators_3_6;
  late final import5.DefaultValueAccessor _DefaultValueAccessor_3_7;
  late final List<import6.ControlValueAccessor<dynamic>> _NgValueAccessor_3_8;
  late final import7.NgModel _NgModel_3_9;
  late final ViewContainer _appEl_4;
  late final NgIf _NgIf_4_9;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_4;
  late final import10.InputElement _el_1;
  static import11.ComponentStyles? _componentStyles;
  ViewG01FormNgModel0(import12.View parentView, int parentIndex) : super(parentView, parentIndex, import13.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import14.unsafeCast(import10.document.createElement('g01-form-ng-model'));
  }
  static String? get _debugComponentUrl {
    return (import14.isDevMode ? 'asset:corpus_ngdart/lib/src/g01_form_ng_model.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import10.document;
    final _el_0 = import15.appendElement<import10.FormElement>(doc, parentRenderNode, 'form');
    this.updateChildClass(_el_0, 'f');
    this._NgForm_0_5 = import2.NgForm(null, this);
    if (import16.isDevToolsEnabled) {
      import16.Inspector.instance.registerDirective(_el_0, this._NgForm_0_5);
    }
    this._el_1 = import15.appendElement<import10.InputElement>(doc, _el_0, 'input');
    import15.setAttribute(this._el_1, 'type', 'text');
    this._RequiredValidator_1_5 = import3.RequiredValidator();
    this._NgValidators_1_6 = [this._RequiredValidator_1_5];
    this._DefaultValueAccessor_1_7 = import5.DefaultValueAccessor(this._el_1);
    this._NgValueAccessor_1_8 = [this._DefaultValueAccessor_1_7];
    this._NgModel_1_9 = import7.NgModel(this._NgValidators_1_6, this._NgValueAccessor_1_8);
    if (import16.isDevToolsEnabled) {
      import16.Inspector.instance.registerDirective(this._el_1, this._RequiredValidator_1_5);
      import16.Inspector.instance.registerDirective(this._el_1, this._DefaultValueAccessor_1_7);
      import16.Inspector.instance.registerDirective(this._el_1, this._NgModel_1_9);
    }
    final _text_2 = import15.appendText(_el_0, ' ');
    final _el_3 = import15.appendElement<import10.TextAreaElement>(doc, _el_0, 'textarea');
    import15.setAttribute(_el_3, 'required', '');
    import15.setAttribute(_el_3, 'rows', '3');
    this._RequiredValidator_3_5 = import3.RequiredValidator();
    this._NgValidators_3_6 = [this._RequiredValidator_3_5];
    this._DefaultValueAccessor_3_7 = import5.DefaultValueAccessor(_el_3);
    this._NgValueAccessor_3_8 = [this._DefaultValueAccessor_3_7];
    this._NgModel_3_9 = import7.NgModel(this._NgValidators_3_6, this._NgValueAccessor_3_8);
    if (import16.isDevToolsEnabled) {
      import16.Inspector.instance.registerDirective(_el_3, this._RequiredValidator_3_5);
      import16.Inspector.instance.registerDirective(_el_3, this._DefaultValueAccessor_3_7);
      import16.Inspector.instance.registerDirective(_el_3, this._NgModel_3_9);
    }
    final _anchor_4 = import15.appendAnchor(_el_0);
    this._appEl_4 = ViewContainer(4, 0, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_G01FormNgModel1);
    this._NgIf_4_9 = NgIf(this._appEl_4, _TemplateRef_4_8);
    if (import16.isDevToolsEnabled) {
      import16.Inspector.instance.registerDirective(_anchor_4, this._NgIf_4_9);
    }
    final _el_5 = import15.appendElement<import10.ButtonElement>(doc, _el_0, 'button');
    import15.setAttribute(_el_5, 'type', 'submit');
    final _text_6 = import15.appendText(_el_5, 'ok');
    _el_0.addEventListener('submit', this.eventHandler1(this._NgForm_0_5.onSubmit));
    _el_0.addEventListener('reset', this.eventHandler1(this._NgForm_0_5.onReset));
    final subscription_0 = this._NgForm_0_5.ngSubmit.listen(this.eventHandler0(_ctx.salvar));
    this._el_1.addEventListener('blur', this.eventHandler0(this._DefaultValueAccessor_1_7.touchHandler));
    this._el_1.addEventListener('input', this.eventHandler1(this._handleEvent_0));
    final subscription_1 = this._NgModel_1_9.update.listen(this.eventHandler1(this._handleEvent_1));
    _el_3.addEventListener('blur', this.eventHandler0(this._DefaultValueAccessor_3_7.touchHandler));
    _el_3.addEventListener('input', this.eventHandler1(this._handleEvent_2));
    final subscription_2 = this._NgModel_3_9.update.listen(this.eventHandler1(this._handleEvent_3));
    this.initSubscriptions([subscription_0, subscription_1, subscription_2]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((nodeIndex <= 6)) {
      if ((1 == nodeIndex)) {
        if (identical(token, const import18.MultiToken<Object>('NgValidators'))) {
          return this._NgValidators_1_6;
        }
        if (identical(token, const import18.MultiToken<import19.ControlValueAccessor<dynamic>>('NgValueAccessor'))) {
          return this._NgValueAccessor_1_8;
        }
        if ((identical(token, import7.NgModel) || identical(token, import20.NgControl))) {
          return this._NgModel_1_9;
        }
      }
      if ((3 == nodeIndex)) {
        if (identical(token, const import18.MultiToken<Object>('NgValidators'))) {
          return this._NgValidators_3_6;
        }
        if (identical(token, const import18.MultiToken<import19.ControlValueAccessor<dynamic>>('NgValueAccessor'))) {
          return this._NgValueAccessor_3_8;
        }
        if ((identical(token, import7.NgModel) || identical(token, import20.NgControl))) {
          return this._NgModel_3_9;
        }
      }
      if ((identical(token, import2.NgForm) || identical(token, import21.ControlContainer))) {
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
    final currVal_1 = (!_ctx.bloqueado);
    if (import22.checkBinding(this._expr_1, currVal_1, '!bloqueado', 'package:corpus_ngdart/src/g01_form_ng_model.html')) {
      if (import16.isDevToolsEnabled) {
        import16.Inspector.instance.recordInput(this._RequiredValidator_1_5, 'required', currVal_1);
      }
      this._RequiredValidator_1_5.required = currVal_1 /* REF:package:corpus_ngdart/src/g01_form_ng_model.html:102:125 */;
      this._expr_1 = currVal_1;
    }
    changed = false;
    final currVal_2 = _ctx.nome;
    if (import22.checkBinding(this._expr_2, currVal_2, 'nome', 'package:corpus_ngdart/src/g01_form_ng_model.html')) {
      if (import16.isDevToolsEnabled) {
        import16.Inspector.instance.recordInput(this._NgModel_1_9, 'ngModel', currVal_2);
      }
      this._NgModel_1_9.model = currVal_2 /* REF:package:corpus_ngdart/src/g01_form_ng_model.html:60:78 */;
      changed = true;
      this._expr_2 = currVal_2;
    }
    if (changed) {
      this._NgModel_1_9.ngAfterChanges();
    }
    if (((!import22.debugThrowIfChanged) && firstCheck)) {
      this._NgModel_1_9.ngOnInit();
    }
    if (firstCheck) {
      if (import16.isDevToolsEnabled) {
        import16.Inspector.instance.recordInput(this._RequiredValidator_3_5, 'required', true);
      }
      this._RequiredValidator_3_5.required = true /* REF:package:corpus_ngdart/src/g01_form_ng_model.html:168:176 */;
    }
    changed = false;
    final currVal_4 = _ctx.texto;
    if (import22.checkBinding(this._expr_4, currVal_4, 'texto', 'package:corpus_ngdart/src/g01_form_ng_model.html')) {
      if (import16.isDevToolsEnabled) {
        import16.Inspector.instance.recordInput(this._NgModel_3_9, 'ngModel', currVal_4);
      }
      this._NgModel_3_9.model = currVal_4 /* REF:package:corpus_ngdart/src/g01_form_ng_model.html:139:158 */;
      changed = true;
      this._expr_4 = currVal_4;
    }
    if (changed) {
      this._NgModel_3_9.ngAfterChanges();
    }
    if (((!import22.debugThrowIfChanged) && firstCheck)) {
      this._NgModel_3_9.ngOnInit();
    }
    if (import16.isDevToolsEnabled) {
      import16.Inspector.instance.recordInput(this._NgIf_4_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_4_9.ngIf = _ctx.mostra /* REF:package:corpus_ngdart/src/g01_form_ng_model.html:196:210 */;
    this._appEl_4.detectChangesInNestedViews();
    final currVal_0 = _ctx.bloqueado;
    if (import22.checkBinding(this._expr_0, currVal_0, 'bloqueado', 'package:corpus_ngdart/src/g01_form_ng_model.html')) {
      import15.setProperty(this._el_1, 'disabled', currVal_0) /* REF:package:corpus_ngdart/src/g01_form_ng_model.html:79:101 */;
      this._expr_0 = currVal_0;
    }
  }

  @override
  void destroyInternal() {
    this._appEl_4.destroyNestedViews();
  }

  void _handleEvent_0($event) {
    this._DefaultValueAccessor_1_7.handleChange($event.target.value);
  }

  void _handleEvent_1($event) {
    final _ctx = this.ctx;
    _ctx.nome = $event;
  }

  void _handleEvent_2($event) {
    this._DefaultValueAccessor_3_7.handleChange($event.target.value);
  }

  void _handleEvent_3($event) {
    final _ctx = this.ctx;
    _ctx.texto = $event;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import11.ComponentStyles.unscoped(styles$G01FormNgModel, _debugComponentUrl));
      if (import14.isDevMode) {
        import11.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _G01FormNgModelNgFactory = ComponentFactory<import1.G01FormNgModel>('g01-form-ng-model', viewFactory_G01FormNgModelHost0);
ComponentFactory<import1.G01FormNgModel> get G01FormNgModelNgFactory {
  return _G01FormNgModelNgFactory;
}

ComponentFactory<import1.G01FormNgModel> createG01FormNgModelFactory() {
  return ComponentFactory('g01-form-ng-model', viewFactory_G01FormNgModelHost0);
}

class _ViewG01FormNgModel1 extends import24.EmbeddedView<import1.G01FormNgModel> {
  late final import5.DefaultValueAccessor _DefaultValueAccessor_1_5;
  late final List<import6.ControlValueAccessor<dynamic>> _NgValueAccessor_1_6;
  late final import7.NgModel _NgModel_1_7;
  Object? _expr_0;
  _ViewG01FormNgModel1(import25.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import14.unsafeCast(doc.createElement('div'));
    final _el_1 = import15.appendElement<import10.InputElement>(doc, _el_0, 'input');
    import15.setAttribute(_el_1, 'type', 'password');
    this._DefaultValueAccessor_1_5 = import5.DefaultValueAccessor(_el_1);
    this._NgValueAccessor_1_6 = [this._DefaultValueAccessor_1_5];
    this._NgModel_1_7 = import7.NgModel(null, this._NgValueAccessor_1_6);
    if (import16.isDevToolsEnabled) {
      import16.Inspector.instance.registerDirective(_el_1, this._DefaultValueAccessor_1_5);
      import16.Inspector.instance.registerDirective(_el_1, this._NgModel_1_7);
    }
    _el_1.addEventListener('blur', this.eventHandler0(this._DefaultValueAccessor_1_5.touchHandler));
    _el_1.addEventListener('input', this.eventHandler1(this._handleEvent_0));
    final subscription_0 = this._NgModel_1_7.update.listen(this.eventHandler1(this._handleEvent_1));
    this.initRootNodesAndSubscriptions(import14.unsafeCast(<Object>[_el_0]), [subscription_0]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((1 == nodeIndex)) {
      if (identical(token, const import18.MultiToken<import19.ControlValueAccessor<dynamic>>('NgValueAccessor'))) {
        return this._NgValueAccessor_1_6;
      }
      if ((identical(token, import7.NgModel) || identical(token, import20.NgControl))) {
        return this._NgModel_1_7;
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
    final currVal_0 = _ctx.senha;
    if (import22.checkBinding(this._expr_0, currVal_0, 'senha', 'package:corpus_ngdart/src/g01_form_ng_model.html')) {
      if (import16.isDevToolsEnabled) {
        import16.Inspector.instance.recordInput(this._NgModel_1_7, 'ngModel', currVal_0);
      }
      this._NgModel_1_7.model = currVal_0 /* REF:package:corpus_ngdart/src/g01_form_ng_model.html:239:258 */;
      changed = true;
      this._expr_0 = currVal_0;
    }
    if (changed) {
      this._NgModel_1_7.ngAfterChanges();
    }
    if (((!import22.debugThrowIfChanged) && firstCheck)) {
      this._NgModel_1_7.ngOnInit();
    }
  }

  void _handleEvent_0($event) {
    this._DefaultValueAccessor_1_5.handleChange($event.target.value);
  }

  void _handleEvent_1($event) {
    final _ctx = this.ctx;
    _ctx.senha = $event;
  }
}

import24.EmbeddedView<void> viewFactory_G01FormNgModel1(import25.RenderView parentView, int parentIndex) {
  return _ViewG01FormNgModel1(parentView, parentIndex);
}

final List<Object> styles$G01FormNgModelHost = const [];

class _ViewG01FormNgModelHost0 extends import26.HostView<import1.G01FormNgModel> {
  @override
  void build() {
    this.componentView = ViewG01FormNgModel0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.G01FormNgModel();
    this.initRootNode(_el_0);
  }
}

import26.HostView<import1.G01FormNgModel> viewFactory_G01FormNgModelHost0() {
  return _ViewG01FormNgModelHost0();
}
