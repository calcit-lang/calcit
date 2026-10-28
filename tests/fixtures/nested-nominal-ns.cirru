
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |nested-nominal
  :entries $ {} $ :default
    {}
      :description "|Nested nominal names in Fn schemas resolve in the declaration namespace (#2027)"
      :init-fn 'quatrefoil.repro/main!
      :mode :native
      :reload-fn 'quatrefoil.repro/main!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'quatrefoil.repro $ %{} 'FileEntry
      :defs $ {}
        'State $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct State (:y 'Number)
          :examples $ []
          :schema $ quote StructDef
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            quatrefoil.types/call! $ fn (state) (println state) &unit
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'read-field $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-field ()
            quatrefoil.types/read-x $ fn (state) (:x state)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |short-name-callback)
            :code $ quote $ do
              assert= 2 $ quatrefoil.repro/read-field
              , &unit
            :tags $ #{} :nested-nominal
        'read-generic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-generic ()
            quatrefoil.types/apply-t 1 $ fn (n) (+ n 1)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |generic-name-stays-a-type-variable)
            :code $ quote $ do
              assert= 2 $ quatrefoil.repro/read-generic
              , &unit
            :tags $ #{} :nested-nominal
        'read-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-list ()
            quatrefoil.types/call-list! $ fn (states)
              :x $ &list:nth states 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |list-of-nominal-callback)
            :code $ quote $ do
              assert= 2 $ quatrefoil.repro/read-list
              , &unit
            :tags $ #{} :nested-nominal
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns quatrefoil.repro
    'quatrefoil.types $ %{} 'FileEntry
      :defs $ {}
        'State $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct State (:x 'Number)
          :examples $ []
          :schema $ quote StructDef
        'T $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct T (:v 'Number)
          :examples $ []
          :schema $ quote StructDef
        'apply-t $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn apply-t (x f) (f x)
          :examples $ []
          :schema $ quote $ Fn T
            Fn T $ -> T
            -> T
            :generics T
        'call! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call! (f)
            f $ State :x 2
            , &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn State $ -> Unit
            -> Unit
        'call-list! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-list! (f)
            f $ [] $ State :x 2
          :examples $ []
          :schema $ quote $ Fn
            Fn (List State) (-> Number)
            -> Number
        'read-x $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-x (f)
            f $ State :x 2
          :examples $ []
          :schema $ quote $ Fn
            Fn State $ -> Number
            -> Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns quatrefoil.types
