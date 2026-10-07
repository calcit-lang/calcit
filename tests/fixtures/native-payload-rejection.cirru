ns app.main $ :require
  calcit.std.time :refer $ set-interval

let
    task $ set-interval 10000 $ fn () (raise |unexpected-emit)
    raw $ :raw task
    attempts $ atom 0
  assert= "|failed to encode async FFI value: AnyRef is not serializable at $" $ try
    task.cancel-with! $ let ()
      reset! attempts $ inc $ deref attempts
      , raw
    fn (error) error
  assert= 1 $ deref attempts
  assert= "|failed to encode async FFI value: AnyRef is not serializable at $.list[1]" $ try
    task.cancel-with! $ [] :reason raw
    fn (error) error
  assert= "|failed to encode async FFI value: AnyRef is not serializable at $.set[0]" $ try
    task.cancel-with! $ #{} raw
    fn (error) error
  assert= "|failed to encode async FFI value: AnyRef is not serializable at $.map[0].key" $ try
    task.cancel-with! $ {} (raw :reason)
    fn (error) error
  assert= "|failed to encode async FFI value: AnyRef is not serializable at $.map[0].value" $ try
    task.cancel-with! $ {} (:reason raw)
    fn (error) error
  assert= "|failed to encode async FFI value: AnyRef is not serializable at $.atom" $ try
    task.cancel-with! $ atom raw
    fn (error) error
  assert= "|failed to encode async FFI value: AnyRef is not serializable at $.struct[:raw]" $ try
    task.cancel-with! task
    fn (error) error
  assert= "|failed to encode async FFI value: AnyRef is not serializable at $.enum[0]" $ try
    task.cancel-with! $ Option :some raw
    fn (error) error
  assert= "|failed to encode async FFI value: AnyRef is not serializable at $.list[0].map[0].value.list[1]" $ try
    task.cancel-with! $ [] $ {} $ :reason $ [] :nested raw
    fn (error) error
  assert= &unit $ task.cancel-with! $ {} (:code :cancelled) (:detail |取消-✓)
  assert= "|failed to encode async FFI value: AnyRef is not serializable at $" $ try
    task.cancel-with! raw
    fn (error) error
  assert= &unit $ task.cancel!
  println |native-payload-rejection-recovered
