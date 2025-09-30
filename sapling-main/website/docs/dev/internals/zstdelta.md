# ZstDelta

ZstDelta uses [zstd](http://www.zstd.net) dictionary compression to calculate
a compressed delta between two inputs.

## ZstDelta

The `zstdelta` Rust library provides `diff` and `apply` to calculate such
compressed deltas and restore content from deltas. You can get `delta` from
`diff(a, b)`, then restore the content of `b` using `apply(a, delta)`.

In Python, `bindings.zstd` provides access to the `diff` and `apply` functions:

```with-output
>>> import bindings, hashlib
>>> a = b"".join(hashlib.sha256(str(i).encode()).digest() for i in range(1000))
>>> len(a)
>>> b = a[:10000] + b'x' * 10000 + a[11000:]
>>> diff = bindings.zstd.diff(a, b)
>>> len(diff)
>>> bindings.zstd.apply(a, diff) == b
```

## ZStore

The `zstore` Rust library provides an on-disk content store with internal
delta-chain management. It uses the above `zstdelta` library for delta
calculation and [IndexedLog](/docs/dev/internals/indexedlog.md) for on-disk storage. It is used by
[MetaLog](/docs/dev/internals/metalog.md).
