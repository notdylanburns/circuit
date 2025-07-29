# New Decl Syntax

Replaces `decl Type foo, bar, baz`

```
circ Foo {
    -> this_is_an_input, input_2;
    <- this_is_an_output;
    <> transput_maybe;

    with {
        Type foo, bar, baz;
        TypeWithArgs(1, MyEnum:Variant, c=4) name;
    }

    with Type foo, bar, baz;
    with TypeWithArgs(1, MyEnum:Variant, c=4) name;
}
```
