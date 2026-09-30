/**
 * @name Hard-coded secret passed to a login call
 * @description A string literal flows into the password argument of `login`.
 * @kind path-problem
 * @problem.severity warning
 * @id py/example/hardcoded-secret
 */

import python
import semmle.python.dataflow.new.DataFlow
import semmle.python.dataflow.new.TaintTracking

/** A call to a function named `login`. */
class LoginCall extends DataFlow::CallCfgNode {
  LoginCall() { this.getFunction().asCfgNode().(NameNode).getId() = "login" }

  DataFlow::Node getPassword() { result = this.getArg(1) or result = this.getArgByName("password") }
}

// Literals that look like real secrets rather than placeholders.
predicate isSuspicious(StringLiteral s) {
  s.getText().length() >= 8 and
  not s.getText().regexpMatch("(?i).*(example|changeme|\\*+).*")
}

module SecretConfig implements DataFlow::ConfigSig {
  predicate isSource(DataFlow::Node n) { isSuspicious(n.asExpr()) }

  predicate isSink(DataFlow::Node n) { exists(LoginCall c | n = c.getPassword()) }
}

module SecretFlow = TaintTracking::Global<SecretConfig>;

import SecretFlow::PathGraph

from SecretFlow::PathNode source, SecretFlow::PathNode sink, int len
where
  SecretFlow::flowPath(source, sink) and
  len = source.getNode().asExpr().(StringLiteral).getText().length() and
  len != 0
select sink.getNode(), source, sink, "Hard-coded secret of length " + len.toString() + " reaches $@.",
  source.getNode(), "this literal"
