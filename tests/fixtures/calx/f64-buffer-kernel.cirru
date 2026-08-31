defn dot-product (left right index acc)
  if (&< index 0)
    , acc
    recur
      , left
      , right
      &- index 1
      &+
        , acc
        &*
          &f64-buffer:get left $ &f64:to-i64-index index
          &f64-buffer:get right $ &f64:to-i64-index index
