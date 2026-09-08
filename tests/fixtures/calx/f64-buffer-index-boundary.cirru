defn index-result (index)
  &f64:to-i64-index index

defn index-local (index)
  &let
    checked $ &f64:to-i64-index index
    &+ checked 1

defn index-arithmetic (index)
  &+ 1 $ &f64:to-i64-index index

defn index-branch (index)
  if true
    &f64:to-i64-index index
    , index

defn index-recur (index)
  if (&< index 1)
    , index
    recur $ &f64:to-i64-index $ &- index 1

defn unchecked-read (buffer index)
  &f64-buffer:get buffer index

defn nested-conversion (buffer index)
  &f64-buffer:get buffer $ &f64:to-i64-index
    &f64:to-i64-index index
