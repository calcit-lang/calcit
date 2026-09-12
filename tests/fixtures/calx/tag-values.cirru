defn tag-id (value)
  , value

defn host-echo (value)
  , value

defn init (flag input)
  &let
    selected $ if flag input :fallback
    tag-id selected

defn reload (input)
  host-echo input
