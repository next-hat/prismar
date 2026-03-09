CREATE TABLE cargoes (
  key TEXT NOT NULL PRIMARY KEY,
  created_at TIMESTAMP NOT NULL,
  name TEXT NOT NULL,
  spec_key TEXT NOT NULL,
  status_key TEXT NOT NULL,
  namespace_name TEXT NOT NULL,
  FOREIGN KEY(namespace_name) REFERENCES namespaces(name)
);
CREATE TABLE namespaces (
  name TEXT NOT NULL PRIMARY KEY
);
