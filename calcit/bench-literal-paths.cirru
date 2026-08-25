
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |bench-literal-paths)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'bench-literal-paths.main/main!) (:mode :native) (:reload-fn 'bench-literal-paths.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |bench-literal-paths.main $ %{} 'FileEntry
      :defs $ {}
        |*path-effects $ %{} 'CodeEntry (:doc |)
          :code $ quote (defatom *path-effects 0)
          :examples $ []
          :schema $ :: 'Dynamic
        |BenchUser $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct BenchUser $ :name 'String
          :examples $ []
          :schema $ :: 'StructDef
        |bench-read-dynamic! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-read-dynamic! () $ println
              loop-read read-dynamic 100000
                &{} :a $ &{} :b 2
                , 0
          :examples $ []
          :schema $ :: 'Dynamic
        |bench-read-typed! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-read-typed! () $ println
              loop-read read-typed 100000
                &{} :a $ &{} :b 2
                , 0
          :examples $ []
          :schema $ :: 'Dynamic
        |bench-write-dynamic! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-write-dynamic! () $ println
              option:unwrap-or
                get-in
                  loop-write write-dynamic 100000 $ &{} :a (&{} :b 2)
                  [] :a :b
                , 0
          :examples $ []
          :schema $ :: 'Dynamic
        |bench-write-typed! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-write-typed! () $ println
              option:unwrap-or
                get-in
                  loop-write write-typed 100000 $ &{} :a (&{} :b 2)
                  [] :a :b
                , 0
          :examples $ []
          :schema $ :: 'Dynamic
        |loop-read $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn loop-read (reader n data acc)
              if (&< n 1) acc $ recur reader (&- n 1) data
                &+ acc $ option:unwrap-or (reader data) 0
          :examples $ []
          :schema $ :: 'Dynamic
        |loop-write $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn loop-write (writer n data)
              if (&< n 1) data $ recur writer (&- n 1) (writer data n)
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () $ let
                data $ &{} :a (&{} :b 2)
              do
                assert= (%some 2) (read-typed data)
                assert= (%some 2) (read-dynamic data)
                assert= (%none)
                  read-typed $ &{}
                assert= (%none)
                  read-dynamic $ &{}
                assert= (%none)
                  read-typed $ unsafe-coerce (&{} :a nil)
                    :: 'Map 'Tag $ :: 'Map 'Tag 'Number
                assert= (%some 2)
                  read-indexed-typed $ [] ([] 1 2)
                assert=
                  %some $ %some 2
                  read-optional-typed $ &{} :value (%some 2)
                assert=
                  %some $ %none
                  read-optional-typed $ &{} :value (%none)
                assert= 3 $ option:unwrap-or
                  get-in (write-typed data 3) ([] :a :b)
                  , 0
                assert= 3 $ option:unwrap-or
                  get-in (write-dynamic data 3) ([] :a :b)
                  , 0
                assert=
                  &{} :a $ &{} :b 3
                  write-typed
                    unsafe-coerce (&{} :a nil)
                      :: 'Map 'Tag $ :: 'Map 'Tag 'Number
                    , 3
                reset! *path-effects 0
                assert= (%none)
                  read-effectful-typed $ &{}
                assert= 12 @*path-effects
                reset! *path-effects 0
                assert=
                  &{} :a $ &{} :b 3
                  write-effectful-typed (&{}) 3
                assert= 123 @*path-effects
                assert= true $ try
                  do
                    read-typed $ unsafe-coerce
                      &{} :a $ %{} BenchUser (:name |Ada)
                      :: 'Map 'Tag $ :: 'Map 'Tag 'Number
                    , false
                  fn (_error) true
          :examples $ []
          :schema $ :: 'Dynamic
        |mark-key! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn mark-key! (step key)
              swap! *path-effects $ fn (current)
                &+ (&* current 10) step
              , key
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ [] 'Number 'Tag
        |mark-value! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn mark-value! (step value)
              swap! *path-effects $ fn (current)
                &+ (&* current 10) step
              , value
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
        |read-dynamic $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn read-dynamic (data)
              get-in data $ [] :a :b
          :examples $ []
          :schema $ :: 'Dynamic
        |read-effectful-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn read-effectful-typed (data)
              get-in data $ [] (mark-key! 1 :a) (mark-key! 2 :b)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
                :: 'Map 'Tag $ :: 'Map 'Tag 'Number
              :return $ :: 'Option 'Number
        |read-indexed-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn read-indexed-typed (data)
              get-in data $ [] 0 1
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
                :: 'List $ :: 'List 'Number
              :return $ :: 'Option 'Number
        |read-optional-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn read-optional-typed (data)
              get-in data $ [] :value
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
                :: 'Map 'Tag $ :: 'Option 'Number
              :return $ :: 'Option (:: 'Option 'Number)
        |read-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn read-typed (data)
              get-in data $ [] :a :b
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
                :: 'Map 'Tag $ :: 'Map 'Tag 'Number
              :return $ :: 'Option 'Number
        |reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
        |write-dynamic $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn write-dynamic (data value)
              assoc-in data ([] :a :b) value
          :examples $ []
          :schema $ :: 'Dynamic
        |write-effectful-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn write-effectful-typed (data value)
              assoc-in data
                [] (mark-key! 1 :a) (mark-key! 2 :b)
                mark-value! 3 value
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
                :: 'Map 'Tag $ :: 'Map 'Tag 'Number
                , 'Number
              :return $ :: 'Map 'Tag (:: 'Map 'Tag 'Number)
        |write-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn write-typed (data value)
              assoc-in data ([] :a :b) value
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
                :: 'Map 'Tag $ :: 'Map 'Tag 'Number
                , 'Number
              :return $ :: 'Map 'Tag (:: 'Map 'Tag 'Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns bench-literal-paths.main $ :require
