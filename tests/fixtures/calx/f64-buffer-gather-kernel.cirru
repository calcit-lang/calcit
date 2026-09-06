defn gather-sum (values indices remaining acc)
  if (&< remaining 1)
    , acc
    &let
      offset $ &- remaining 1
      recur
        , values
        , indices
        , offset
        &+
          , acc
          &f64-buffer:get values $ &f64:to-i64-index
            &f64-buffer:get indices $ &f64:to-i64-index offset
