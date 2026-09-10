defn string-id (value)
  , value

defn host-echo (value)
  , value

defn init (flag input)
  &let
    selected $ if flag input |fallback
    string-id selected

defn reload (input)
  host-echo input
