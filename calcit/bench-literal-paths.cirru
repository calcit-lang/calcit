{} (:package |bench-literal-paths)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'bench-literal-paths.main/main!) (:mode :native) (:reload-fn 'bench-literal-paths.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |bench-literal-paths.main $ %{} 'FileEntry
      :defs $ {}
        |read-dynamic $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn read-dynamic (data)
              get-in data $ [] :a :b
          :examples $ []
          :schema $ :: 'Dynamic
        |read-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn read-typed (data)
              get-in data $ [] :a :b
          :examples $ []
          :schema $ :: 'Fn
            {} (:return $ :: 'Option 'Number)
              :args $ []
                :: 'Map 'Tag $ :: 'Map 'Tag 'Number
        |write-dynamic $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn write-dynamic (data value)
              assoc-in data ([] :a :b) value
          :examples $ []
          :schema $ :: 'Dynamic
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
              :return $ :: 'Map 'Tag $ :: 'Map 'Tag 'Number
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
        |bench-read-typed! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-read-typed! ()
              println $ loop-read read-typed 100000 (&{} :a $ &{} :b 2) 0
          :examples $ []
          :schema $ :: 'Dynamic
        |bench-read-dynamic! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-read-dynamic! ()
              println $ loop-read read-dynamic 100000 (&{} :a $ &{} :b 2) 0
          :examples $ []
          :schema $ :: 'Dynamic
        |bench-write-typed! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-write-typed! ()
              println $ option:unwrap-or
                get-in
                  loop-write write-typed 100000 $ &{} :a $ &{} :b 2
                  [] :a :b
                , 0
          :examples $ []
          :schema $ :: 'Dynamic
        |bench-write-dynamic! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn bench-write-dynamic! ()
              println $ option:unwrap-or
                get-in
                  loop-write write-dynamic 100000 $ &{} :a $ &{} :b 2
                  [] :a :b
                , 0
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              let
                  data $ &{} :a $ &{} :b 2
                do
                  assert= (%some 2) (read-typed data)
                  assert= (%some 2) (read-dynamic data)
                  assert= (%none) (read-typed (&{}))
                  assert= (%none) (read-dynamic (&{}))
                  assert= 3 $ option:unwrap-or (get-in (write-typed data 3) ([] :a :b)) 0
                  assert= 3 $ option:unwrap-or (get-in (write-dynamic data 3) ([] :a :b)) 0
                  println |typed-read $ loop-read read-typed 100000 data 0
                  println |dynamic-read $ loop-read read-dynamic 100000 data 0
                  println |typed-write $ option:unwrap-or (get-in (loop-write write-typed 100000 data) ([] :a :b)) 0
                  println |dynamic-write $ option:unwrap-or (get-in (loop-write write-dynamic 100000 data) ([] :a :b)) 0
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns bench-literal-paths.main $ :require
