
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-union
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-union.main/main!) (:mode :native) (:reload-fn 'test-union.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'test-union.main $ %{} 'FileEntry
      :defs $ {}
        'AliasValue $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion AliasValue test-union.main/PointAlias test-union.main/ShapeAlias String
          :examples $ []
          :schema $ quote UnionDef
        'CellPatch $ %{} 'CodeEntry
          :doc "|Enum payloads are checked to their leaves when the enum value is built."
          :code $ quote $ defenum CellPatch (:set Cells)
          :examples $ []
          :schema $ quote EnumDef
          :tests $ [] $ %{} 'TestEntry (:name |payload-elements-validate)
            :code $ quote $ do
              assert= |accepted $ try
                do
                  %:: CellPatch :set $ erased-cells $ segment-raw ([] 1 2)
                  , |accepted
                fn (e) |rejected
              assert= |rejected $ try
                do
                  %:: CellPatch :set $ erased-cells $ segment-raw ([] 1 |bad)
                  , |accepted
                fn (e) |rejected
            :tags $ #{} :union :unit
        'Cells $ %{} 'CodeEntry
          :doc "|A member written as a compound type, which keeps its quote."
          :code $ quote $ defunion Cells String (List Number)
          :examples $ []
          :schema $ quote UnionDef
        'CssValue $ %{} 'CodeEntry (:doc "|A style value: text or a number of pixels.")
          :code $ quote $ defunion CssValue String Number
          :examples $ []
          :schema $ quote UnionDef
        'Geo $ %{} 'CodeEntry (:doc "|Members told apart by their definitions.")
          :code $ quote $ defunion Geo Point Shape String
          :examples $ []
          :schema $ quote UnionDef
        'Json $ %{} 'CodeEntry
          :doc "|A recursive union: lists and maps hold Json values again."
          :code $ quote $ defunion Json String Number Bool (List Json) (Map String Json)
          :examples $ []
          :schema $ quote UnionDef
        'Label $ %{} 'CodeEntry (:doc "|Every member of Label is also a member of Geo.")
          :code $ quote $ defunion Label String Point
          :examples $ []
          :schema $ quote UnionDef
        'LabelA $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct LabelA (:text String)
          :examples $ []
          :schema $ quote StructDef
        'LabelB $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct LabelB (:text String)
          :examples $ []
          :schema $ quote StructDef
        'LabelText $ %{} 'CodeEntry
          :doc "|Two struct members with one map shape: encode-map-as must refuse what decode-map-as cannot tell apart (#2562)."
          :code $ quote $ defunion LabelText LabelA LabelB
          :examples $ []
          :schema $ quote UnionDef
          :tests $ [] $ %{} 'TestEntry (:name |encode-rejects-ambiguous-struct-members)
            :code $ quote $ do
              assert=
                {} $ :text |a
                encode-map-as (LabelA :text |a) LabelText
              assert= "|encode-map-as failed at $: the encoded value would decode as another union member; the union has members that share one shape" $ try
                do
                  encode-map-as (LabelB :text |b) LabelText
                  , |unexpected-success
                fn (message)
                  hint-fn $ Fn String $ -> String
                  , message
              assert= "|encode-map-as failed at $[0]: the encoded value would decode as another union member; the union has members that share one shape" $ try
                do
                  encode-map-as
                    [] (LabelB :text |b) 1
                    quote $ Tuple LabelText Number
                  , |unexpected-success
                fn (message)
                  hint-fn $ Fn String $ -> String
                  , message
            :tags $ #{} :union :unit
        'Loop $ %{} 'CodeEntry
          :doc "|A union whose `Ref` member points back to itself, so a ref may hold itself."
          :code $ quote $ defunion Loop Number (Ref Loop)
          :examples $ []
          :schema $ quote UnionDef
        'LoopBox $ %{} 'CodeEntry
          :doc "|Checking a payload that holds a self-holding ref stops at the cycle."
          :code $ quote $ defenum LoopBox (:hold Loop)
          :examples $ []
          :schema $ quote EnumDef
          :tests $ [] $ %{} 'TestEntry (:name |self-holding-ref-validates)
            :code $ quote $ assert= |accepted
              try
                do
                  %:: LoopBox :hold $ erased-loop $ self-holding-ref
                  , |accepted
                fn (e) |rejected
            :tags $ #{} :union :unit
        'MaybeGeo $ %{} 'CodeEntry
          :doc "|A Geo, or an Option of one, as optional child arguments are."
          :code $ quote $ defunion MaybeGeo Geo (Option Geo)
          :examples $ []
          :schema $ quote UnionDef
        'Nest $ %{} 'CodeEntry
          :doc "|A payload that refers back to its own enum is checked once the enum is defined."
          :code $ quote $ defenum Nest (:leaf Number)
            :many $ List Nest
          :examples $ []
          :schema $ quote EnumDef
          :tests $ [] $ %{} 'TestEntry (:name |recursive-payload-validates)
            :code $ quote $ assert= 2
              match
                Nest :many $ [] (Nest :leaf 1)
                  Nest :many $ [] $ Nest :leaf 2
                (:many xs) (count xs)
                (:leaf _) 0
            :tags $ #{} :union :unit
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x Number) (:y Number)
          :examples $ []
          :schema $ quote StructDef
        'PointAlias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def PointAlias Point
          :examples $ []
          :schema $ quote StructDef
        'RouteSegment $ %{} 'CodeEntry
          :doc "|A route pattern segment: literal text, a Symbol parameter, or a Tag parameter."
          :code $ quote $ defunion RouteSegment String Symbol Tag
          :examples $ []
          :schema $ quote UnionDef
        'Scale $ %{} 'CodeEntry
          :doc "|A scale: one factor for both axes, or one per axis."
          :code $ quote $ defunion Scale Number (List Number)
          :examples $ []
          :schema $ quote UnionDef
        'Segment $ %{} 'CodeEntry
          :doc "|A route pattern segment: literal text or a parameter name (respo-router)."
          :code $ quote $ defunion Segment String Symbol
          :examples $ []
          :schema $ quote UnionDef
        'Shape $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Shape (:circle Number) (:square Number)
          :examples $ []
          :schema $ quote EnumDef
        'ShapeAlias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def ShapeAlias Shape
          :examples $ []
          :schema $ quote EnumDef
        'Style $ %{} 'CodeEntry
          :doc "|A struct field typed as a union accepts any member at runtime."
          :code $ quote $ defstruct Style (:width CssValue)
          :examples $ []
          :schema $ quote StructDef
        'StylePatch $ %{} 'CodeEntry
          :doc "|An enum payload typed by a union written with its short name."
          :code $ quote $ defenum StylePatch (:width CssValue) (:reset)
          :examples $ []
          :schema $ quote EnumDef
          :tests $ [] $ %{} 'TestEntry (:name |short-union-payload-validates)
            :code $ quote $ do
              assert= |12px $ patch-text $ StylePatch :width 12
              assert= |auto $ patch-text $ StylePatch :width |auto
            :tags $ #{} :union :unit
        'Tree $ %{} 'CodeEntry
          :doc "|A tree of numbers; the union refers to itself inside its List member."
          :code $ quote $ defunion Tree Number (List Tree)
          :examples $ []
          :schema $ quote UnionDef
        'TripleInput $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion TripleInput Number (Tuple Number Number Number)
          :examples $ []
        'cells-total $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn cells-total (c)
            match c
              (String s) (count s)
              (List xs) (reduce xs 0 &+)
          :examples $ []
          :schema $ quote $ Fn Cells (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |compound-member-keeps-element-type)
            :code $ quote $ do
              assert= 6 $ cells-total $ [] 1 2 3
              assert= 3 $ cells-total |abc
            :tags $ #{} :union :unit
        'css-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn css-text (v)
            match v
              (String s) s
              (Number n) (str n |px)
          :examples $ []
          :schema $ quote $ Fn CssValue (-> String)
          :tests $ [] $ %{} 'TestEntry (:name |member-values-flow-into-the-union)
            :code $ quote $ do
              assert= |12px $ css-text 12
              assert= |auto $ css-text |auto
            :tags $ #{} :union :unit
        'describe $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn describe (g)
            match g
              (Point p)
                str "|point " $ :x p
              (Shape s)
                match s
                  (:circle r) (str "|circle " r)
                  (:square w) (str "|square " w)
              (String t) t
          :examples $ []
          :schema $ quote $ Fn Geo (-> String)
          :tests $ [] $ %{} 'TestEntry (:name |struct-and-enum-members)
            :code $ quote $ do
              assert= "|point 1" $ describe $ Point :x 1 :y 2
              assert= "|circle 3" $ describe $ Shape :circle 3
              assert= |plain $ describe |plain
            :tags $ #{} :union :unit
        'describe-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn describe-alias (value)
            match value
              (PointAlias point)
                str "|point " $ :x point
              (test-union.main/ShapeAlias shape)
                match shape
                  (:circle radius) (str "|circle " radius)
                  (:square width) (str "|square " width)
              (String text) text
          :examples $ []
          :schema $ quote $ Fn test-union.main/AliasValue (-> String)
          :tests $ [] $ %{} 'TestEntry (:name |nominal-alias-patterns)
            :code $ quote $ do
              assert= "|point 1" $ describe-alias $ PointAlias :x 1 :y 2
              assert= "|circle 3" $ describe-alias $ ShapeAlias :circle 3
              assert= |plain $ describe-alias |plain
            :tags $ #{} :union :unit
        'describe-all $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn describe-all (xs)
            .map xs $ fn (x) (describe x)
          :examples $ []
          :schema $ quote $ Fn (List Geo)
            -> $ List String
        'describe-label $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn describe-label (l) (describe l)
          :examples $ []
          :schema $ quote $ Fn Label (-> String)
        'describe-maybe $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn describe-maybe (& items)
            map items $ fn (item)
              match item
                (Option o)
                  .unwrap-or (.map o describe) |absent
                _ |present
          :examples $ []
          :schema $ quote $ Fn (& MaybeGeo)
            -> $ List String
          :tests $ []
            %{} 'TestEntry (:name |option-member-matches-separately)
              :code $ quote $ do
                assert= ([] |present |plain |absent |absent)
                  describe-maybe |a (Option :some |plain) (Option :none) (nil->option nil)
                assert= ([] "|point 1")
                  describe-maybe $ Option :some $ Point :x 1 :y 2
              :tags $ #{} :union :unit
            %{} 'TestEntry (:name |option-member-decodes-from-open-data)
              :code $ quote $ do
                assert= ([] |absent)
                  describe-maybe $ decode-map-as
                    segment-raw $ Option :none
                    , MaybeGeo
                assert= ([] |plain)
                  describe-maybe $ decode-map-as
                    segment-raw $ Option :some |plain
                    , MaybeGeo
              :tags $ #{} :union :unit
        'erased-cells $ %{} 'CodeEntry
          :doc "|Restores a value from a type-erased cache without checking it, so `%::` has to."
          :code $ quote $ defn erased-cells (x) (unsafe-coerce x 'Cells)
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Cells) (:features :erased-cache)
        'erased-loop $ %{} 'CodeEntry
          :doc "|Restores a Loop from a type-erased cache without checking it, so `%::` has to."
          :code $ quote $ defn erased-loop (x) (unsafe-coerce x 'Loop)
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Loop) (:features :erased-cache)
        'json-size $ %{} 'CodeEntry (:doc "|Counts every node of a Json value.")
          :code $ quote $ defn json-size (j)
            match j
              (List xs)
                foldl xs 1 $ fn (acc x)
                  &+ acc $ json-size x
              (Map m)
                foldl (&map:vals m) 1 $ fn (acc x)
                  &+ acc $ json-size x
              _ 1
          :examples $ []
          :schema $ quote $ Fn Json (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |walks-nested-members)
            :code $ quote $ do
              assert= 1 $ json-size |a
              assert= 5 $ json-size $ {} (|a 1)
                |b $ [] |x true
              assert= 5 $ json-size $ parse-cirru-edn-as "|{} (|a 1) (|b $ [] |x true)" Json
              assert= true $ .err? $ try-parse-cirru-edn-as "|{} (|a $ [] :tag)" Json
            :tags $ #{} :union :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (println "|Testing union...")
            assert= |12px $ css-text 12
            assert= |auto $ css-text |auto
            assert= "|point 1" $ describe $ Point :x 1 :y 2
            assert= "|circle 3" $ describe $ Shape :circle 3
            assert= "|square 4" $ describe $ Shape :square 4
            assert= |plain $ describe |plain
            assert= "|point 3" $ describe $ pick 3
            assert= |none $ describe $ pick 0
            assert= "|point 5" $ describe-label $ Point :x 5 :y 0
            assert= |text $ describe-label |text
            assert= ([] |a "|square 2")
              describe-all $ [] |a $ Shape :square 2
            assert= |other $ text-only 1
            assert= |s $ text-only |s
            assert= |12px $ css-text $ :width (Style :width 12)
            assert= |auto $ css-text $ :width (Style :width |auto)
            assert= |12px $ patch-text $ StylePatch :width 12
            assert= ([] |present |absent "|circle 2")
              describe-maybe (Point :x 1 :y 1) (Option :none)
                Option :some $ Shape :circle 2
            assert= 10 $ tree-total $ [] 1 ([] 2 3)
              [] $ [] 4
            assert= 5 $ json-size $ [] |a ([] 1 false)
            assert= ([] 2 2)
              scale-xy $ read-scale 2
            assert= ([] 1 3)
              scale-xy $ read-scale $ [] 1 3
            assert= false $ scale-decodes? |x
            assert= false $ scale-decodes? $ [] 1 |a
            assert= true $ scale-decodes? 1
            assert= "|point 1" $ describe $ read-geo
              {} (:x 1) (:y 2)
            assert= "|circle 3" $ describe $ read-geo (Shape :circle 3)
            assert= |plain $ describe $ read-geo |plain
            assert= ([] 4 5)
              scale-xy $ parse-cirru-edn-as "|[] 4 5" Scale
            assert= ([] 6 6)
              scale-xy $ parse-cirru-edn-as "|do 6" Scale
            assert= "|point 7" $ describe $ parse-cirru-edn-as "|%{} :Point (:x 7) (:y 0)" Geo
            assert= true $ .err? $ try-parse-cirru-edn-as "|do :a" Scale
            assert= |text:home $ segment-text $ decode-map-as (segment-raw |home) Segment
            assert= |param:id $ segment-text $ decode-map-as (segment-raw 'id) Segment
            assert= (Point :x 1 :y 2)
              round-trip-geo $ Point :x 1 :y 2
            assert= (Shape :circle 3)
              round-trip-geo $ Shape :circle 3
            assert= |plain $ round-trip-geo |plain
            assert=
              {} (:x 1) (:y 2)
              encode-map-as (Point :x 1 :y 2) Geo
            println "|Union tests passed"
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'nullable-css-values $ %{} 'CodeEntry
          :doc "|A union value lifts into JsNullish of the same union, also inside an immutable Map (#2559)."
          :code $ quote $ defn nullable-css-values (xs) xs
          :examples $ []
          :schema $ quote $ Fn (Map Tag CssValue)
            -> $ Map Tag $ JsNullish CssValue
          :tests $ [] $ %{} 'TestEntry (:name |union-lifts-into-js-nullish)
            :code $ quote $ assert=
              {} (:width 1) (:color |red)
              nullable-css-values $ {} (:width 1) (:color |red)
            :tags $ #{} :union :unit
        'patch-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn patch-text (p)
            match p
              (:width w) (css-text w)
              (:reset) |reset
          :examples $ []
          :schema $ quote $ Fn StylePatch (-> String)
        'pick $ %{} 'CodeEntry (:doc "|Both branches check against the declared union.")
          :code $ quote $ defn pick (n)
            if (> n 0) (Point :x n :y 2) |none
          :examples $ []
          :schema $ quote $ Fn Number (-> Geo)
        'read-geo $ %{} 'CodeEntry
          :doc "|Checked decode into a union whose members include a struct and an enum."
          :code $ quote $ defn read-geo (raw) (decode-map-as raw Geo)
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Geo)
          :tests $ [] $ %{} 'TestEntry (:name |map-decodes-to-struct-member)
            :code $ quote $ do
              assert= "|point 1" $ describe $ read-geo
                {} (:x 1) (:y 2)
              assert= "|circle 3" $ describe $ read-geo (Shape :circle 3)
              assert= |plain $ describe $ read-geo |plain
            :tags $ #{} :union :unit
        'read-scale $ %{} 'CodeEntry (:doc "|Checked decode of open data into a union.")
          :code $ quote $ defn read-scale (raw) (decode-map-as raw Scale)
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Scale)
          :tests $ [] $ %{} 'TestEntry (:name |decodes-each-member)
            :code $ quote $ do
              assert= ([] 2 2)
                scale-xy $ read-scale 2
              assert= ([] 1 3)
                scale-xy $ read-scale $ [] 1 3
            :tags $ #{} :union :unit
        'read-triple $ %{} 'CodeEntry
          :doc "|Checked decode into a union with a fixed-length Tuple member (#2557)."
          :code $ quote $ defn read-triple (raw)
            match (decode-map-as raw TripleInput)
              (Number n) ([] n n n)
              (Tuple t) t
          :examples $ []
          :schema $ quote $ Fn Dynamic
            -> $ Tuple Number Number Number
          :tests $ [] $ %{} 'TestEntry (:name |decodes-tuple-member)
            :code $ quote $ do
              assert= ([] 2 2 2) (read-triple 2)
              assert= ([] 1 2 3)
                read-triple $ [] 1 2 3
              let[] (x y z)
                read-triple $ [] 4 5 6
                assert= 15 $ + x y z
              assert= false $ .ok? $ try-decode-map-as ([] 1 2) TripleInput
              assert= false $ .ok? $ try-decode-map-as ([] 1 2 3 4) TripleInput
              assert= false $ .ok? $ try-decode-map-as ([] 1 :b 3) TripleInput
              assert= ([] 1 |a)
                parse-cirru-edn-as "|[] 1 |a" $ quote $ Tuple Number String
              assert= false $ .ok? $ try-parse-cirru-edn-as "|[] 1"
                quote $ Tuple Number String
            :tags $ #{} :union :unit
        'round-trip-geo $ %{} 'CodeEntry
          :doc "|encode-map-as is the inverse of decode-map-as for each union member (#2562)."
          :code $ quote $ defn round-trip-geo (g)
            decode-map-as (encode-map-as g Geo) Geo
          :examples $ []
          :schema $ quote $ Fn Geo (-> Geo)
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-each-member)
            :code $ quote $ do
              assert= (Point :x 1 :y 2)
                round-trip-geo $ Point :x 1 :y 2
              assert=
                {} (:x 1) (:y 2)
                encode-map-as (Point :x 1 :y 2) Geo
              assert= (Shape :circle 3)
                round-trip-geo $ Shape :circle 3
              assert= |plain $ round-trip-geo |plain
              assert= (Option :none)
                decode-map-as
                  encode-map-as (Option :none) MaybeGeo
                  , MaybeGeo
              let
                  some-point $ Option :some $ Point :x 1 :y 2
                  nested $ Option :some $ Option :none
                assert= some-point $ decode-map-as (encode-map-as some-point MaybeGeo) MaybeGeo
                assert= nested $ decode-map-as
                  encode-map-as nested $ quote $ Option MaybeGeo
                  quote $ Option MaybeGeo
              let[] (cell geo)
                decode-map-as
                  encode-map-as
                    [] (ref 3) |plain
                    quote $ Tuple (Ref Number) Geo
                  quote $ Tuple (Ref Number) Geo
                assert= 3 $ deref cell
                assert= |plain geo
            :tags $ #{} :union :unit
        'scale-decodes? $ %{} 'CodeEntry
          :doc "|A member kind that matches but holds the wrong elements still fails."
          :code $ quote $ defn scale-decodes? (raw)
            .ok? $ try-decode-map-as raw Scale
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Bool)
        'scale-xy $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn scale-xy (s)
            match s
              (Number n) ([] n n)
              (List xs) xs
          :examples $ []
          :schema $ quote $ Fn Scale
            -> $ List Number
        'segment-kinds $ %{} 'CodeEntry
          :doc "|Symbol literals are inferred as Symbol, so a literal pattern list such as ([] |team 'team-id :id) is checked as List RouteSegment."
          :code $ quote $ defn segment-kinds (segments)
            map segments $ fn (s)
              match s
                (String _) |text
                (Symbol _) |param
                (Tag _) |tag
          :examples $ []
          :schema $ quote $ Fn (List RouteSegment)
            -> $ List String
          :tests $ [] $ %{} 'TestEntry (:name |symbol-literal-is-route-segment)
            :code $ quote $ do
              assert= ([] |text |param |tag)
                segment-kinds $ [] |team 'team-id :id
              assert= ([] |param)
                segment-kinds $ [] 'id
            :tags $ #{} :union :unit
        'segment-raw $ %{} 'CodeEntry
          :doc "|Hands a value over as open data, as a route table read from a Dynamic list would."
          :code $ quote $ defn segment-raw (x) x
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Dynamic)
        'segment-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn segment-text (s)
            match s
              (String t) (str |text: t)
              (Symbol p)
                str |param: $ to-string p
          :examples $ []
          :schema $ quote $ Fn Segment (-> String)
        'self-holding-ref $ %{} 'CodeEntry (:doc "|Builds a ref whose content is the ref itself.")
          :code $ quote $ defn self-holding-ref ()
            let
                r $ ref $ segment-raw 0
              reset! r $ segment-raw r
              segment-raw r
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'text-only $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn text-only (v)
            match v
              (String s) s
              _ |other
          :examples $ []
          :schema $ quote $ Fn CssValue (-> String)
        'tree-total $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tree-total (t)
            match t
              (Number n) n
              (List xs)
                foldl xs 0 $ fn (acc x)
                  &+ acc $ tree-total x
          :examples $ []
          :schema $ quote $ Fn Tree (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |sums-nested-lists)
            :code $ quote $ do
              assert= 5 $ tree-total 5
              assert= 10 $ tree-total $ [] 1 ([] 2 3)
                [] $ [] 4
              assert= 6 $ tree-total $ parse-cirru-edn-as "|[] 1 ([] 2 ([] 3))" Tree
            :tags $ #{} :union :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-union.main
    'test-union.member-origin $ %{} 'FileEntry
      :defs $ {}
        'GenericCell $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct GenericCell ([] T) (:value T)
          :examples $ []
          :schema $ quote StructDef
        'GenericMembers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion GenericMembers Scalar (GenericCell Number) (GenericPacket Number)
          :examples $ []
        'GenericPacket $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum GenericPacket ([] T) (:payload T)
          :examples $ []
          :schema $ quote EnumDef
        'Inner $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Inner String (List Scalar)
          :examples $ []
        'Scalar $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Scalar String Number Tag
          :examples $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-union.member-origin
    'test-union.member-shadow $ %{} 'FileEntry
      :defs $ {}
        'Outer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Outer test-union.member-origin/Inner Tag
          :examples $ []
          :tests $ []
            %{} 'TestEntry (:name |nested-members-keep-declaration-namespace)
              :code $ quote $ do
                match
                  decode-map-as ([] 12 :block |sans) Outer
                  (List items)
                    assert= ([] 12 :block |sans) items
                  _ $ raise |Expected-list-from-nested-union
                match (parse-cirru-edn-as "|[] 12 :block |sans" Outer)
                  (List items)
                    assert= ([] 12 :block |sans) items
                  _ $ raise |Expected-list-from-nested-union
                match
                  try-decode-map-as ([] false 'bad) Outer
                  (:err error) &unit
                  (:ok value) (raise |Accepted-shadowed-invalid-member)
                match (try-parse-cirru-edn-as "|[] false 'bad" Outer)
                  (:err error) &unit
                  (:ok value) (raise |Accepted-shadowed-invalid-member)
              :tags $ #{} :union :unit
            %{} 'TestEntry
              :name |applied-generic-members-keep-declaration-scope-and-payload-types
              :code $ quote $ do
                assert=
                  format-cirru-edn $ test-union.member-origin/GenericCell :value 12
                  format-cirru-edn $ parse-cirru-edn-as "|%{} :GenericCell (:value 12)" test-union.member-origin/GenericMembers
                assert=
                  format-cirru-edn $ test-union.member-origin/GenericPacket :payload 12
                  format-cirru-edn $ parse-cirru-edn-as "|%:: :GenericPacket :payload 12" test-union.member-origin/GenericMembers
                assert= true $ match
                  try-decode-map-as
                    {} $ :value 12
                    , test-union.member-origin/GenericMembers
                  (:ok value) true
                  (:err error) false
                assert= true $ match
                  try-decode-map-as (test-union.member-origin/GenericPacket :payload 12) test-union.member-origin/GenericMembers
                  (:ok value) true
                  (:err error) false
                assert= true $ match
                  try-decode-map-as
                    {} $ :value |bad
                    , test-union.member-origin/GenericMembers
                  (:err error) true
                  (:ok value) false
                assert= true $ match
                  try-decode-map-as (test-union.member-origin/GenericPacket :payload |bad) test-union.member-origin/GenericMembers
                  (:err error) true
                  (:ok value) false
                assert= true $ match (try-parse-cirru-edn-as "|%{} :GenericCell (:value |bad)" test-union.member-origin/GenericMembers)
                  (:err error) true
                  (:ok value) false
                assert= true $ match (try-parse-cirru-edn-as "|%:: :GenericPacket :payload |bad" test-union.member-origin/GenericMembers)
                  (:err error) true
                  (:ok value) false
                assert= true $ match (try-decode-map-as false test-union.member-origin/GenericMembers)
                  (:err error) true
                  (:ok value) false
              :tags $ #{} :union :unit
        'Scalar $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Scalar Bool Symbol
          :examples $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-union.member-shadow
