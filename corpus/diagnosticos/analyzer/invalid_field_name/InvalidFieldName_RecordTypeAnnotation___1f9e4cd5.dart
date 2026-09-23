void f((int, String, {int $2}) r) {}
//                        ^^
// [diag.invalidFieldNamePositional] Record field names can't be a dollar sign followed by an integer when the integer is the index of a positional field.
