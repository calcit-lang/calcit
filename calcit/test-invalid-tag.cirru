
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-invalid-tag
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'test-invalid-tag.main/main!
      :mode :native
      :reload-fn 'test-invalid-tag.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'test-invalid-tag.main
    %{} 'FileEntry
      :defs $ {}
        'Result $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Result (:err 'String) (:ok)
          :examples $ []
          :schema $ :: 'EnumDef
        'ResultImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl ResultImpl ResultTrait
            .dummy $ fn $ _x
          :examples $ []
          :schema $ :: 'Impl
        'ResultTrait $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait ResultTrait
            .dummy $ :: :fn $ {}
              :generics $ [] 'T
              :args $ [] 'T
              :return 'Unit
          :examples $ []
          :schema $ :: 'Trait
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println "|Testing %:: call"
            println |Result: Result
            println |ResultImpl: ResultImpl
              ; Direct call to %:: to see if function is invoked
              println "|Calling %:: ..."
            let
                result $ %:: Result :invalid
              println "|Should not reach here:" result
          :examples $ []
          :schema $ :: 'Dynamic
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! ()
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-invalid-tag.main
