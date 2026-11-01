
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-union
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-union.main/main!) (:mode :native) (:reload-fn 'test-union.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'test-union.main
    %{} 'FileEntry
      :defs $ {}
        'Cells $ %{} 'CodeEntry
          :doc "|A member written as a compound type, which keeps its quote."
          :code $ quote $ defunion Cells String
            quote $ List Number
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
        'Label $ %{} 'CodeEntry (:doc "|Every member of Label is also a member of Geo.")
          :code $ quote $ defunion Label String Point
          :examples $ []
          :schema $ quote UnionDef
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x Number) (:y Number)
          :examples $ []
          :schema $ quote StructDef
        'Scale $ %{} 'CodeEntry
          :doc "|A scale: one factor for both axes, or one per axis."
          :code $ quote $ defunion Scale Number
            quote $ List Number
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
        'Style $ %{} 'CodeEntry
          :doc "|A struct field typed as a union accepts any member at runtime."
          :code $ quote $ defstruct Style (:width CssValue)
          :examples $ []
          :schema $ quote StructDef
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
            println "|Union tests passed"
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
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
        'segment-raw $ %{} 'CodeEntry
          :doc "|Hands a value over as open data, as a route table read from a Dynamic list would."
          :code $ quote $ defn segment-raw (x) x
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Dynamic)
        'segment-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn segment-text (s)
            match s
              (String t) (str |text: t)
              (Symbol p) (str |param: (to-string p))
          :examples $ []
          :schema $ quote $ Fn Segment (-> String)
        'text-only $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn text-only (v)
            match v
              (String s) s
              _ |other
          :examples $ []
          :schema $ quote $ Fn CssValue (-> String)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-union.main
