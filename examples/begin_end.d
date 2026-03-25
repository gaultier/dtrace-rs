BEGIN {
  print("begin");
}

END {
  c = '\n';
  @ = count();
  print(c);
  print("end");
}
