defn range-sum (n acc)
  if (&< n 1)
    , acc
    recur (&- n 1) (&+ acc n)

defn fibonacci (n)
  if (&< n 2)
    , n
    &+
      fibonacci $ &- n 1
      fibonacci $ &- n 2

defn affine-helper (x scale offset)
  &let
    scaled $ &* x scale
    &+ scaled offset

defn affine (x scale offset)
  affine-helper x scale offset

defn polynomial (x)
  &+
    &* 3 $ &* x x
    &+ (&* 2 x) 1

defn bounded-simulation (remaining state rate)
  if (&< remaining 1)
    , state
    recur
      &- remaining 1
      &+ (&* state rate) 0.001
      , rate
