typedef struct  {int somefield;} mytype;
BEGIN {
  print(offsetof( mytype, somefield));
}
