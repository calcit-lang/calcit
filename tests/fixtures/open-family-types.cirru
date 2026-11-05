
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {}
      :description "|AnyFn, AnyStruct and AnyEnum are the bare family types, written on purpose (#2594)."
      :init-fn 'app.main/main!
      :mode :native
      :reload-fn 'app.main/main!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'Holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Holder (:callback AnyFn) (:item AnyStruct) (:op AnyEnum)
          :examples $ []
          :schema $ quote StructDef
        'Op $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Op (:add Number) (:reset)
          :examples $ []
          :schema $ quote EnumDef
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x Number)
          :examples $ []
          :schema $ quote StructDef
        'apply-any $ %{} 'CodeEntry
          :doc "|Calls a function of any arity, like a memo helper."
          :code $ quote $ defn apply-any (f args) (apply f args)
          :examples $ []
          :schema $ quote $ Fn AnyFn (List Dynamic) (-> Dynamic)
        'call-any $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-any (f x) (f x)
          :examples $ []
          :schema $ quote $ Fn AnyFn Dynamic (-> Dynamic)
        'checked-assoc $ %{} 'CodeEntry
          :doc "|A dynamic field write: the struct and key are not known statically."
          :code $ quote $ defn checked-assoc (base key value)
            if (struct? base)
              try (&struct:assoc base key value)
                fn (_error) nil
              , nil
          :examples $ []
          :schema $ quote $ Fn Dynamic Dynamic Dynamic (-> Dynamic)
        'listen $ %{} 'CodeEntry
          :doc "|A library callback that carries enums the application defines."
          :code $ quote $ defn listen (dispatch!)
            dispatch! $ :: :reset
          :examples $ []
          :schema $ quote $ Fn
            Fn AnyEnum $ -> Unit
            -> Unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
          :tests $ []
            %{} 'TestEntry (:name |any-fn-calls-and-applies)
              :code $ quote $ do
                assert= 2 $ call-any inc 1
                assert= 3 $ apply-any &+ $ [] 1 2
                assert= 6 $ apply-any
                  fn (a b c)
                    &+ a $ &+ b c
                  [] 1 2 3
              :tags $ #{} :open-family :unit
            %{} 'TestEntry (:name |any-struct-reflects)
              :code $ quote $ let
                  a $ %{} Point $ :x 1
                  b $ %{} Point $ :x 2
                assert= true $ same-struct? a b
                assert= 1 $ struct-size a
              :tags $ #{} :open-family :unit
            %{} 'TestEntry (:name |fields-keep-any-member)
              :code $ quote $ let
                  holder $ %{} Holder (:callback inc)
                    :item $ %{} Point $ :x 1
                    :op $ %:: Op :reset
                  changed $ .assoc holder :callback dec
                assert= 0 $ call-any (:callback changed) 1
                assert= true $ same-struct? (:item holder)
                  %{} Point $ :x 3
                assert= (%:: Op :reset) (:op holder)
              :tags $ #{} :open-family :unit
            %{} 'TestEntry (:name |dynamic-writes-check-the-family)
              :code $ quote $ let
                  holder $ %{} Holder (:callback inc)
                    :item $ %{} Point $ :x 1
                    :op $ %:: Op :reset
                assert= true $ struct? $ checked-assoc holder :callback dec
                assert= nil $ checked-assoc holder :callback 1
                assert= nil $ checked-assoc holder :item 1
                assert= nil $ checked-assoc holder :op 1
              :tags $ #{} :open-family :unit
            %{} 'TestEntry (:name |enum-callbacks-stay-compatible)
              :code $ quote $ let
                  *seen $ ref $ :: :none
                listen $ fn (op)
                  hint-fn $ Fn Enum $ -> Unit
                  reset! *seen op
                  , &unit
                assert= (:: :reset) (deref *seen)
              :tags $ #{} :open-family :unit
        'same-struct? $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn same-struct? (a b) (&struct:matches? a b)
          :examples $ []
          :schema $ quote $ Fn AnyStruct AnyStruct (-> Bool)
        'struct-size $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn struct-size (value) (&struct:count value)
          :examples $ []
          :schema $ quote $ Fn AnyStruct (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
