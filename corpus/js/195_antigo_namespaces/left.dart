int _value() { return 11; }
int left() { var _value = 3; return _value + local(); }
int local() { return _value(); }
