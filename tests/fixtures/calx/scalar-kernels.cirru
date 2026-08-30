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
  &+ (&* x scale) offset

defn affine (x scale offset)
  affine-helper x scale offset
