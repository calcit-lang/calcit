
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |broad-type-candidates
  :entries $ {} $ :default
    {}
      :description "|W_BROAD_TYPE names a concrete type when every checked call passes one (#2566)."
      :init-fn 'broad-type-candidates.main/main!
      :mode :native
      :reload-fn 'broad-type-candidates.main/main!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'broad-type-candidates.main
    %{} 'FileEntry
      :defs $ {}
        'Msg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Msg (:text String)
          :examples $ []
          :schema $ quote EnumDef
        'Op $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Op (:add Number) (:reset)
          :examples $ []
          :schema $ quote EnumDef
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x Number)
          :examples $ []
          :schema $ quote StructDef
        'comp-button $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defcomp comp-button (d!)
            d! $ %:: Op :reset
          :examples $ []
          :schema $ quote $ Fn
            Fn Enum $ -> Unit
            -> Unit
        'defcomp $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro defcomp (comp-name params & body)
            quasiquote $ defn ~comp-name ~params ~@body
          :examples $ []
          :schema $ quote $ Macro SyntaxSymbol SyntaxList & Syntax
            -> $ Definition Fn
        'dispatch! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn dispatch! (op) &unit
          :examples $ []
          :schema $ quote $ Fn Enum (-> Unit)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            dispatch! $ %:: Op :add 1
            dispatch! $ %:: Op :reset
            mixed! $ %:: Op :reset
            mixed! $ %:: Msg :text |a
            show-point $ %{} Point $ :x 1
            partly-anonymous! $ %:: Op :reset
            partly-anonymous! $ :: :ping
            on-click dispatch!
            on-shadowed dispatch!
            comp-button dispatch!
            spread-op! $ %:: Op :reset
            let
                ops $ [] (%:: Op :add 1)
              spread-op! & ops
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'mixed! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn mixed! (op) &unit
          :examples $ []
          :schema $ quote $ Fn Enum (-> Unit)
        'on-click $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn on-click (d!)
            d! $ %:: Op :add 1
            d! $ %:: Op :reset
            , &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn Enum $ -> Unit
            -> Unit
        'on-shadowed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn on-shadowed (d!)
            d! $ %:: Op :reset
            let
                d! $ fn (m)
                  hint-fn $ Fn Enum (-> Unit)
                  , &unit
              d! $ %:: Msg :text |a
          :examples $ []
          :schema $ quote $ Fn
            Fn Enum $ -> Unit
            -> Unit
        'partly-anonymous! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn partly-anonymous! (op) &unit
          :examples $ []
          :schema $ quote $ Fn Enum (-> Unit)
        'show-point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn show-point (p) &unit
          :examples $ []
          :schema $ quote $ Fn Struct (-> Unit)
        'spread-op! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn spread-op! (op) &unit
          :examples $ []
          :schema $ quote $ Fn Enum (-> Unit)
        'unused! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unused! (op) &unit
          :examples $ []
          :schema $ quote $ Fn Enum (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns broad-type-candidates.main
