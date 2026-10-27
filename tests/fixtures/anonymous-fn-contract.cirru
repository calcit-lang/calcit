
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'async-callback $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn (current previous)
            hint-fn $ quote $ Fn Number Number (-> Unit) (:async)
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] 'Number 'Number
        'async-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn ()
            hint-fn $ {} $ :async true
            , 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'count-items $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn (items) (.count items)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'T
            :generics $ [] 'T
            :where $ {} $ 'T 'Countable
          :tests $ [] $ %{} 'TestEntry (:name |uses-definition-trait-bound)
            :code $ quote $ let
                numbers $ [] 1 2 3
                text |hello
              assert= 3 $ count-items numbers
              assert= 5 $ count-items text
            :tags $ #{} :fn-contract :unit
        'increment $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn (n) (&+ n 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |uses-definition-input-context)
            :code $ quote $ assert= 8 (increment 7)
            :tags $ #{} :fn-contract :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'make-echo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn (seed)
            fn (value)
              hint-fn $ quote $ Fn String (-> String)
              , value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Number
            :return $ :: 'Fn $ {} (:return 'String)
              :args $ [] 'String
          :tests $ [] $ %{} 'TestEntry (:name |keeps-nested-function-context-independent)
            :code $ quote $ let
                echo-text $ make-echo 7
              assert= |text $ echo-text |text
            :tags $ #{} :fn-contract :unit
        'option-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ fn (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Option 'Nil
            :return $ :: 'Option 'Nil
          :tests $ [] $ %{} 'TestEntry (:name |preserves-none-and-some-nil)
            :code $ quote $ let
                present $ Option :some nil
                absent $ Option :none
              assert= present $ option-value present
              assert= absent $ option-value absent
              assert= true $ identical? present $ option-value present
              assert= true $ identical? absent $ option-value absent
            :tags $ #{} :fn-contract :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
