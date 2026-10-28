
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |bench-literal-paths
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'bench-literal-paths.main/main!) (:mode :native) (:reload-fn 'bench-literal-paths.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'bench-literal-paths.main
    %{} 'FileEntry
      :defs $ {}
        '*path-effects $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *path-effects 0
          :examples $ []
          :schema $ quote $ Ref Number
        'BenchUser $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct BenchUser (:name 'String)
          :examples $ []
          :schema $ quote StructDef
        'bench-read-dynamic! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bench-read-dynamic! ()
            println $ loop-read read-dynamic 100000
              &{} :a $ &{} :b 2
              , 0
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'bench-read-typed! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bench-read-typed! ()
            println $ loop-read read-typed 100000
              &{} :a $ &{} :b 2
              , 0
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'bench-write-dynamic! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bench-write-dynamic! ()
            println $ .unwrap-or
              get-in
                loop-write write-dynamic 100000 $ &{} :a $ &{} :b 2
                [] :a :b
              , 0
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'bench-write-typed! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bench-write-typed! ()
            println $ .unwrap-or
              get-in
                loop-write write-typed 100000 $ &{} :a $ &{} :b 2
                [] :a :b
              , 0
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'loop-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn loop-read (reader n data acc)
            if (&< n 1) acc $ recur reader (&- n 1) data $ &+ acc
              .unwrap-or (reader data) 0
          :examples $ []
          :schema $ quote $ Fn
            Fn Dynamic $ -> $ Result Dynamic PathError
            , Number Dynamic Number (-> Number)
        'loop-write $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn loop-write (writer n data)
            if (&< n 1) data $ recur writer (&- n 1) (writer data n)
          :examples $ []
          :schema $ quote $ Fn
            Fn Dynamic Number $ -> Dynamic
            , Number Dynamic (-> Dynamic)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                data $ &{} :a $ &{} :b 2
                missing $ Result :err $ PathError :missing
              do
                assert= (Result :ok 2) (read-typed data)
                assert= (Result :ok 2) (read-dynamic data)
                assert= missing $ read-typed $ &{}
                assert= missing $ read-dynamic $ &{}
                assert= (Result :ok 2)
                  read-indexed-typed $ [] $ [] 1 2
                assert=
                  Result :ok $ Option :some 2
                  read-optional-typed $ &{} :value $ Option :some 2
                assert=
                  Result :ok $ Option :none
                  read-optional-typed $ &{} :value $ Option :none
                assert= 3 $ .unwrap-or
                  get-in (write-typed data 3) ([] :a :b)
                  , 0
                assert= 3 $ .unwrap-or
                  get-in (write-dynamic data 3) ([] :a :b)
                  , 0
                reset! *path-effects 0
                assert= missing $ read-effectful-typed $ &{}
                assert= 12 @*path-effects
                reset! *path-effects 0
                assert=
                  &{} :a $ &{} :b 3
                  write-effectful-typed (&{}) 3
                assert= 123 @*path-effects
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'mark-key! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn mark-key! (step key)
            reset! *path-effects $ &+
              &* (deref *path-effects) 10
              , step
            , key
          :examples $ []
          :schema $ quote $ Fn Number Tag (-> Tag)
        'mark-value! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn mark-value! (step value)
            reset! *path-effects $ &+
              &* (deref *path-effects) 10
              , step
            , value
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'read-dynamic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-dynamic (data)
            get-in data $ [] :a :b
          :examples $ []
          :schema $ quote $ Fn Dynamic
            -> $ Result Dynamic PathError
        'read-effectful-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-effectful-typed (data)
            get-in data $ [] (mark-key! 1 :a) (mark-key! 2 :b)
          :examples $ []
          :schema $ quote $ Fn
            Map Tag $ Map Tag Number
            -> $ Result Number PathError
        'read-indexed-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-indexed-typed (data)
            get-in data $ [] 0 1
          :examples $ []
          :schema $ quote $ Fn
            List $ List Number
            -> $ Result Number PathError
        'read-optional-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-optional-typed (data)
            get-in data $ [] :value
          :examples $ []
          :schema $ quote $ Fn
            Map Tag $ Option Number
            -> $ Result (Option Number) PathError
        'read-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-typed (data)
            get-in data $ [] :a :b
          :examples $ []
          :schema $ quote $ Fn
            Map Tag $ Map Tag Number
            -> $ Result Number PathError
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'write-dynamic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-dynamic (data value)
            assoc-in data ([] :a :b) value
          :examples $ []
          :schema $ quote $ Fn Dynamic Dynamic (-> Dynamic)
        'write-effectful-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-effectful-typed (data value)
            assoc-in data
              [] (mark-key! 1 :a) (mark-key! 2 :b)
              mark-value! 3 value
          :examples $ []
          :schema $ quote $ Fn
            Map Tag $ Map Tag Number
            , Number
              -> $ Map Tag $ Map Tag Number
        'write-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-typed (data value)
            assoc-in data ([] :a :b) value
          :examples $ []
          :schema $ quote $ Fn
            Map Tag $ Map Tag Number
            , Number
              -> $ Map Tag $ Map Tag Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns bench-literal-paths.main (:require)
