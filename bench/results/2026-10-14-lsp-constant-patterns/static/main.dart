class C { static const threshold = 1; static int read(int x) => switch (x) { threshold => 42, _ => 0 }; }
