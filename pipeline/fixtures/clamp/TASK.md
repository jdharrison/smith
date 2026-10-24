# Clamp fixture task

Change only `clamp.py`. Implement `clamp(value, low, high)` for finite numeric
inputs: return `low` if `value < low`, `high` if `value > high`, otherwise
`value`. Bounds are inclusive. Equal bounds return that bound. If `low > high`,
raise `ValueError` regardless of `value`.

The supplied baseline is deliberately incorrect. The trusted checker and its
cases live outside this candidate directory; candidate-supplied tests or check
reports are not evidence.

This is a restricted pure-function fixture, not arbitrary Python. Supply exactly
one undecorated function with those three positional parameters, no defaults or
annotations. Allowed statements: `if`/`elif`/`else`, `return`, and
`raise ValueError()` (no arguments). Allowed expressions: parameter names,
bounded integer/float literals, unary `+`/`-`, comparisons (`< <= > >= == !=`),
`and`/`or`/`not`, and conditional expressions. No imports, attributes,
assignments, arithmetic, loops, collections, arbitrary calls or other statements.
Comments and whitespace are allowed; docstrings are not. See the trusted
checker/README for exact byte, node, depth and numeric limits.
