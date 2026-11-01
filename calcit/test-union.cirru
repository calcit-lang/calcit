
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
        'CssValue $ %{} 'CodeEntry (:doc "|A style value: text or a number of pixels.")
          :code $ quote $ defunion CssValue 'String 'Number
          :examples $ []
          :schema $ quote UnionDef
        'Geo $ %{} 'CodeEntry (:doc "|Members told apart by their definitions.")
          :code $ quote $ defunion Geo 'Point 'Shape 'String
          :examples $ []
          :schema $ quote UnionDef
        'Label $ %{} 'CodeEntry (:doc "|Every member of Label is also a member of Geo.")
          :code $ quote $ defunion Label 'String 'Point
          :examples $ []
          :schema $ quote UnionDef
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x 'Number) (:y 'Number)
          :examples $ []
          :schema $ quote StructDef
        'Shape $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Shape (:circle 'Number) (:square 'Number)
          :examples $ []
          :schema $ quote EnumDef
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
            println "|Union tests passed"
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'pick $ %{} 'CodeEntry (:doc "|Both branches check against the declared union.")
          :code $ quote $ defn pick (n)
            if (> n 0) (Point :x n :y 2) |none
          :examples $ []
          :schema $ quote $ Fn Number (-> Geo)
        'text-only $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn text-only (v)
            match v
              (String s) s
              _ |other
          :examples $ []
          :schema $ quote $ Fn CssValue (-> String)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-union.main
