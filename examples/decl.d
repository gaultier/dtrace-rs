struct Person {int age; int id;};

enum { Red, Green };

enum Color {Purple};

struct Color {int x;};

enum Color {Orange};

union  PersonOrColor { struct Person p; enum Color c;} ;

BEGIN {
  print(offsetof(struct Person, id));
  print(offsetof(union PersonOrColor, c));
}
