# Macros

Builtin functions to apply inputs to circs more easily.
Maybe determine a syntax for user creation too.

```
circ MyAnd {
    -> a[2];
    <- q;

    and(a[1], a[2]) -> q;
}
```
