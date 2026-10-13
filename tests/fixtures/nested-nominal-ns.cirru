{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |nested-nominal
  :entries $ {} $ :default
    {} (:description "|Nested nominal names in Fn schemas resolve in the declaration namespace (#2027)") (:init-fn 'quatrefoil.repro/main!) (:mode :native) (:reload-fn 'quatrefoil.repro/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'quatrefoil.repro $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            quatrefoil.types/call! $ fn (state)
              println state
              , &unit
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'State $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct State (:y 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'read-generic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-generic () $ quatrefoil.types/apply-t 1
            fn (n) (+ n 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |generic-name-stays-a-type-variable)
            :code $ quote $ do
              assert= 2 $ quatrefoil.repro/read-generic
              , &unit
            :tags $ #{} :nested-nominal
        'read-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-list () $ quatrefoil.types/call-list!
            fn (states) (:x $ &list:nth states 0)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |list-of-nominal-callback)
            :code $ quote $ do
              assert= 2 $ quatrefoil.repro/read-list
              , &unit
            :tags $ #{} :nested-nominal
        'read-field $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-field () $ quatrefoil.types/read-x
            fn (state) (:x state)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |short-name-callback)
            :code $ quote $ do
              assert= 2 $ quatrefoil.repro/read-field
              , &unit
            :tags $ #{} :nested-nominal
      :ns $ %{} 'CodeEntry (:doc |)
        :code $ quote $ ns quatrefoil.repro
        :examples $ []
    'quatrefoil.types $ %{} 'FileEntry
      :defs $ {}
        'State $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct State (:x 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'T $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct T (:v 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'apply-t $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn apply-t (x f) (f x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'T)
            :args $ [] 'T
              :: 'Fn $ {} (:return 'T)
                :args $ [] 'T
            :generics $ [] 'T
        'call-list! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-list! (f)
            f $ [] $ State :x 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
              :: 'Fn $ {} (:return 'Number)
                :args $ [] $ :: 'List 'State
        'call! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call! (f)
            f $ State :x 2
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
              :: 'Fn $ {} (:return 'Unit)
                :args $ [] 'State
        'read-x $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-x (f)
            f $ State :x 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
              :: 'Fn $ {} (:return 'Number)
                :args $ [] 'State
      :ns $ %{} 'CodeEntry (:doc |)
        :code $ quote $ ns quatrefoil.types
        :examples $ []
