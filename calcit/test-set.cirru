
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-set
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-set.main/main!) (:mode :native) (:reload-fn 'test-set.main/reload!)
      :feature-policy $ {}
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {} $ 'test-set.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (log-title "|Testing set") (test-method-dispatch) (do true)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'test-method-dispatch $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-method-dispatch ()
            assert= (#{} 1 2 3)
              .add (#{} 1 2) 3
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-set.main
          :require $ util.core :refer $ log-title
