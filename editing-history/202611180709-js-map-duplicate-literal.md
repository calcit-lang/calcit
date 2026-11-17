# JS Map duplicate keys

Calcit Map construction keeps the last value for equal keys. The JS slice-backed
Map retained duplicate pairs, making its length and lookup disagree with native
and WASM. Normalize pairs at construction so all operations see one entry per
key; preserve the first position and replace the value. Small inputs use Calcit
value equality directly, while larger inputs group by the existing Calcit hash
and still check equality within each bucket to handle collisions.

This adds work during construction, especially for larger maps. It changes no
syntax or type contract. Definition-attached core tests exercise duplicate keys
across native, JS and WASM; JS collision and structural-key behavior also need
verification because the hash buckets are a backend implementation detail.

Typed EDN has a separate contract: differently written source keys that decode
to equal typed keys must be rejected. Its intermediate map view therefore
preserves source entries until after decoding, like its existing set view. Exact
repeated source keys retain their existing parser behavior.
