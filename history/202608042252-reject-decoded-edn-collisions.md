# Reject decoded EDN collection collisions

## Change summary

- Strict typed EDN set decoding now rejects distinct source items that decode to the same Calcit value, reporting the colliding `$.item` path.
- Strict typed EDN map decoding now rejects distinct source keys that decode to the same Calcit key, reporting the colliding `$.key` path before decoding or overwriting the associated value.
- Regression tests cover set-value and map-key collisions caused by record field-order normalization, along with successful decoding when collection entries remain unique.

## Knowledge point

EDN collection uniqueness is not sufficient after typed decoding because normalization can make distinct EDN values equal as Calcit values. Collision checks must therefore use the decoded collection's equality semantics immediately before insertion.
