int read(int x) => switch (x) { missing => 1, _ => 0 };
int other() { const missing = 1; return missing; }
