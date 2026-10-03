
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |fix-command
  :entries $ {} $ :default
    {} (:description "|Compiler-guided source fix fixture.") (:init-fn 'fix-command.main/main!) (:mode :native) (:reload-fn 'fix-command.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'fix-command.main
    %{} 'FileEntry
      :defs $ {}
        'Message $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Message (:empty) (:pair 'String 'Number)
          :examples $ []
          :schema $ :: 'EnumDef
        'Person $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Person (:name 'String) (:age 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'checked-open-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn checked-open-count (value)
            if (list? value) (count value) (raise |expected-countable-list)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |narrowed-list-counts)
            :code $ quote $ do
              assert= 0 $ checked-open-count $ []
              assert= 3 $ checked-open-count $ [] 1 2 3
            :tags $ #{} :count-contract :unit
        'local-bound-counts $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn local-bound-counts ()
            let
                counted $ fn (value)
                  hint-fn $ {}
                    :args $ [] 'T
                    :generics $ [] 'T
                    :where $ {} $ 'T 'Countable
                    :return 'Number
                  count value
              assert= 3 $ counted $ [] 1 2 3
              assert= 0 $ counted $ []
              , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |preserves-expanded-bound)
            :code $ quote $ assert= &unit (local-bound-counts)
            :tags $ #{} :count-contract :unit
        'main! $ %{} 'CodeEntry (:doc |Entry.)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'nominal-counts $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn nominal-counts (person message)
            [] (count person) (count message)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'fix-command.main/Person 'fix-command.main/Message
            :return $ :: 'List 'Number
          :tests $ [] $ %{} 'TestEntry (:name |field-and-tag-counts)
            :code $ quote $ do
              assert= ([] 2 3)
                nominal-counts (Person :name |Ada :age 42) (Message :pair |x 42)
              assert= ([] 2 1)
                nominal-counts (Person :name |Bob :age 0) (Message :empty)
            :tags $ #{} :count-contract :unit
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'typed-loop-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn typed-loop-count (values)
            loop
                remaining values
                index 0
                seen $ assert-type ([]) (:: List Number)
              if (empty? remaining) (count seen)
                recur (rest remaining) (inc index) (append seen index)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'List 'Number
          :tests $ [] $ %{} 'TestEntry (:name |preserves-initializer-types)
            :code $ quote $ do
              assert= 0 $ typed-loop-count $ []
              assert= 3 $ typed-loop-count $ [] 10 20 30
            :tags $ #{} :count-contract :unit
        'typed-rest-forward $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn typed-rest-forward (xs)
            let
                sink $ fn (label & values)
                  hint-fn $ {}
                    :args $ [] 'String
                    :rest 'Number
                    :return 'Number
                  count values
              sink |numbers & xs
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'List 'Number
          :tests $ [] $ %{} 'TestEntry (:name |nonempty-and-empty)
            :code $ quote $ do
              assert= 3 $ typed-rest-forward $ [] 1 2 3
              assert= 0 $ typed-rest-forward $ []
            :tags $ #{} :unit
      :ns $ %{} 'NsEntry (:doc "|Fix command fixture.")
        :code $ quote $ ns fix-command.main
