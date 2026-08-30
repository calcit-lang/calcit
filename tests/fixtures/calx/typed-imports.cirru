defn host-scale (x)
  &* x 2

defn host-observe (x)
  , &unit

defn host-trap (x)
  , x

defn imported-pipeline (x)
  &let
    scaled $ assert-type (host-scale x) 'Number
    host-observe scaled
    &+ scaled 1

defn imported-trap (x)
  host-trap x
